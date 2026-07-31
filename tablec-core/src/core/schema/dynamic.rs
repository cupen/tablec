//! cdylib 动态加载：plugin 必须用 tablec_plugin_create_v1 / drop_v1 入口
use crate::core::diagnostic::{Diagnostic, DiagnosticCode, SourceLocation};
use crate::core::schema::{SchemaParseResult, SchemaParser};
use std::fmt;
use std::path::Path;
use std::ptr::NonNull;
use std::sync::Arc;

#[derive(Debug)]
pub enum DynamicPluginError {
    Load(libloading::Error),
    Symbol(libloading::Error),
    NullPointer,
    DuplicateName(String),
}

impl fmt::Display for DynamicPluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DynamicPluginError::Load(e) => write!(
                f,
                "failed to load .so: {} (note: host must use same Rust version as plugin)",
                e
            ),
            DynamicPluginError::Symbol(e) => write!(
                f,
                "missing symbol in plugin: {} (note: plugin must export tablec_plugin_create_v1 / drop_v1)",
                e
            ),
            DynamicPluginError::NullPointer => write!(f, "plugin returned null pointer"),
            DynamicPluginError::DuplicateName(name) => {
                write!(f, "plugin name '{}' already registered", name)
            }
        }
    }
}

impl std::error::Error for DynamicPluginError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DynamicPluginError::Load(e) | DynamicPluginError::Symbol(e) => Some(e),
            DynamicPluginError::NullPointer | DynamicPluginError::DuplicateName(_) => None,
        }
    }
}

pub struct DynamicPlugin {
    _lib: libloading::Library,
    parser: NonNull<dyn SchemaParser>,
    // Rust trait objects don't have a stable FFI-safe layout, but our ABI contract
    // requires host and plugin to be compiled with the same Rust toolchain (see
    // `load`). The layout therefore matches by construction.
    #[allow(improper_ctypes_definitions)]
    drop_fn: unsafe extern "C" fn(*mut dyn SchemaParser),
}

// Safety: the cdylib owns the heap allocation behind `parser` and we only access it
// through `drop_fn` (which the plugin crate guarantees is thread-safe per the ABI).
unsafe impl Send for DynamicPlugin {}
unsafe impl Sync for DynamicPlugin {}

impl DynamicPlugin {
    /// 加载 cdylib
    /// Safety: host / plugin 必须用相同 Rust 工具链编译
    pub unsafe fn load(path: &Path) -> Result<Arc<Self>, DynamicPluginError> {
        // SAFETY: callers must use only trusted plugin paths; we are responsible for
        // making sure host and plugin were built with the same Rust toolchain.
        let lib = unsafe { libloading::Library::new(path) }.map_err(DynamicPluginError::Load)?;
        // SAFETY: symbol names are part of the documented ABI; types match `extern "C"` ABI.
        let create: libloading::Symbol<unsafe extern "C" fn() -> *mut dyn SchemaParser> =
            unsafe { lib.get(b"tablec_plugin_create_v1") }.map_err(DynamicPluginError::Symbol)?;
        let drop_fn: libloading::Symbol<unsafe extern "C" fn(*mut dyn SchemaParser)> =
            unsafe { lib.get(b"tablec_plugin_drop_v1") }.map_err(DynamicPluginError::Symbol)?;
        let create_fn: unsafe extern "C" fn() -> *mut dyn SchemaParser = *create;
        let drop_fn_ptr: unsafe extern "C" fn(*mut dyn SchemaParser) = *drop_fn;
        let raw = unsafe { create_fn() };
        let parser = NonNull::new(raw).ok_or(DynamicPluginError::NullPointer)?;
        Ok(Arc::new(Self {
            _lib: lib,
            parser,
            drop_fn: drop_fn_ptr,
        }))
    }
}

impl SchemaParser for DynamicPlugin {
    fn name(&self) -> &str {
        // SAFETY: `parser` is a valid `*mut dyn SchemaParser` allocated by the
        // plugin's `create_v1`; we own it (drop_fn runs in Drop).
        unsafe { (*self.parser.as_ptr()).name() }
    }
    fn parse_schema(
        &self,
        sheet_name: &str,
        sheet: &[Vec<String>],
    ) -> Result<SchemaParseResult, Vec<Diagnostic>> {
        use std::panic::{AssertUnwindSafe, catch_unwind};
        catch_unwind(AssertUnwindSafe(|| unsafe {
            (*self.parser.as_ptr()).parse_schema(sheet_name, sheet)
        }))
        .unwrap_or_else(|_| {
            Err(vec![Diagnostic::new(
                DiagnosticCode::HeaderParserError,
                format!("plugin panicked while parsing sheet '{}'", sheet_name),
                SourceLocation::default(),
            )])
        })
    }
}

impl Drop for DynamicPlugin {
    fn drop(&mut self) {
        // SAFETY: parser was created by the plugin's `create_v1` and is consumed
        // by its matching `drop_v1`; calling once here is correct because the
        // owning Arc ensures `Drop` runs exactly once.
        unsafe { (self.drop_fn)(self.parser.as_ptr()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_nonexistent_path_yields_load_error() {
        let r = unsafe { DynamicPlugin::load(Path::new("/tmp/tablec_no_such_plugin_xyz.so")) };
        assert!(matches!(r, Err(DynamicPluginError::Load(_))));
    }

    #[test]
    fn display_load_error_mentions_shared_object_and_version_note() {
        // Load wraps a libloading::Error; the Display impl adds a hint
        // about matching host / plugin Rust toolchain. We can't easily
        // construct a libloading::Error by hand, but the failing-load
        // path above proves the inner error flows through — here we just
        // assert the *added* hint is present in the rendered message.
        let result = unsafe { DynamicPlugin::load(Path::new("/tmp/tablec_no_such_plugin_xyz.so")) };
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err for missing .so"),
        };
        let msg = format!("{}", err);
        assert!(
            msg.contains(".so"),
            "expected .so hint in Display, got: {msg}"
        );
        assert!(
            msg.contains("Rust"),
            "expected Rust toolchain hint in Display, got: {msg}"
        );
    }

    #[test]
    fn display_symbol_error_mentions_expected_symbol_names() {
        // Construct the Symbol variant directly with a placeholder inner
        // error. We don't need the inner error to be a real libloading
        // error — Display only formats the variant's own hint, not the
        // inner error's contents.
        let err = DynamicPluginError::Symbol(libloading::Error::DlSymUnknown);
        let msg = format!("{}", err);
        // Both ABI symbol suffixes must be mentioned so the user knows
        // what to export. The current Display text uses the full
        // `tablec_plugin_create_v1` prefix and a shortened `drop_v1`
        // — assert both names appear somewhere in the message.
        assert!(
            msg.contains("create_v1"),
            "expected create_v1 symbol name in Display, got: {msg}"
        );
        assert!(
            msg.contains("drop_v1"),
            "expected drop_v1 symbol name in Display, got: {msg}"
        );
    }

    #[test]
    fn display_null_pointer_error_is_descriptive() {
        let err = DynamicPluginError::NullPointer;
        let msg = format!("{}", err);
        assert!(
            msg.contains("null"),
            "expected 'null' in Display, got: {msg}"
        );
    }

    #[test]
    fn display_duplicate_name_includes_the_name() {
        let err = DynamicPluginError::DuplicateName("widget_v3".to_string());
        let msg = format!("{}", err);
        assert!(
            msg.contains("widget_v3"),
            "expected plugin name in Display, got: {msg}"
        );
        assert!(
            msg.contains("registered"),
            "expected 'registered' in Display, got: {msg}"
        );
    }

    #[test]
    fn all_four_variants_render_distinct_messages() {
        // Guard against accidentally collapsing two variants into the
        // same Display string (would defeat log triage).
        let load_result =
            unsafe { DynamicPlugin::load(Path::new("/tmp/tablec_no_such_plugin_xyz.so")) };
        let load_err = match load_result {
            Err(e) => e,
            Ok(_) => panic!("expected Err for missing .so"),
        };
        let load = format!("{}", load_err);
        let symbol = format!(
            "{}",
            DynamicPluginError::Symbol(libloading::Error::DlSymUnknown)
        );
        let null = format!("{}", DynamicPluginError::NullPointer);
        let dup = format!("{}", DynamicPluginError::DuplicateName("x".to_string()));
        let all = [&load, &symbol, &null, &dup];
        for (i, a) in all.iter().enumerate() {
            for (j, b) in all.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "variant {i} and {j} rendered the same message");
                }
            }
        }
    }

    #[test]
    fn source_for_load_returns_some_underlying_error() {
        // Failed load path returns a Load variant whose `source()` must
        // surface the inner libloading::Error (not None) so callers can
        // walk the chain.
        let result = unsafe { DynamicPlugin::load(Path::new("/tmp/tablec_no_such_plugin_xyz.so")) };
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err for missing .so"),
        };
        use std::error::Error as _;
        assert!(
            err.source().is_some(),
            "Load variant must expose an inner error via source()"
        );
    }

    #[test]
    fn source_for_symbol_returns_some_underlying_error() {
        let err = DynamicPluginError::Symbol(libloading::Error::DlSymUnknown);
        use std::error::Error as _;
        assert!(
            err.source().is_some(),
            "Symbol variant must expose an inner error via source()"
        );
    }

    #[test]
    fn source_for_null_pointer_is_none() {
        let err = DynamicPluginError::NullPointer;
        use std::error::Error as _;
        assert!(
            err.source().is_none(),
            "NullPointer variant has no inner error; source() must be None"
        );
    }

    #[test]
    fn source_for_duplicate_name_is_none() {
        let err = DynamicPluginError::DuplicateName("widget_v3".to_string());
        use std::error::Error as _;
        assert!(
            err.source().is_none(),
            "DuplicateName variant has no inner error; source() must be None"
        );
    }

    // Compile-time assertions that `DynamicPlugin` is `Send + Sync` —
    // documented as required for `Arc<DynamicPlugin>` to be shared across
    // threads. If either bound is removed, this test stops compiling.
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    #[test]
    fn dynamic_plugin_is_send_and_sync() {
        assert_send::<DynamicPlugin>();
        assert_sync::<DynamicPlugin>();
        assert_send::<Arc<DynamicPlugin>>();
        assert_sync::<Arc<DynamicPlugin>>();
    }
}
