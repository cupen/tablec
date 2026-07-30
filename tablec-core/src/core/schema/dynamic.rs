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
        }
    }
}

impl std::error::Error for DynamicPluginError {}

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
}
