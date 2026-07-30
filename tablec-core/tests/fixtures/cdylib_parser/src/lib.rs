//! cdylib 端到端 fixture：编译为 .so 后被 DynamicPlugin::load 加载
//!
//! 行为与 StandardSchemaParser 完全一致 — 测试目标只是验证动态加载链路本身
//! （符号导出 → libloading → SchemaParser trait dispatch → 表格解析）。
use tablec_core::core::diagnostic::Diagnostic;
use tablec_core::core::schema::{SchemaParseResult, SchemaParser, StandardSchemaParser};

pub struct FixtureParser;

impl SchemaParser for FixtureParser {
    fn name(&self) -> &str {
        "fixture"
    }

    fn parse_schema(
        &self,
        sheet_name: &str,
        sheet: &[Vec<String>],
    ) -> Result<SchemaParseResult, Vec<Diagnostic>> {
        StandardSchemaParser.parse_schema(sheet_name, sheet)
    }
}

/// FFI 入口：plugin ABI v1
///
/// # Safety
///
/// 进程初始化阶段由 host 调用一次；返回的指针必须用 `tablec_plugin_drop_v1` 释放。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tablec_plugin_create_v1() -> *mut dyn SchemaParser {
    Box::into_raw(Box::new(FixtureParser))
}

/// FFI 释放入口
///
/// # Safety
///
/// 必须且仅能由 `tablec_plugin_create_v1` 返回的指针调用一次。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tablec_plugin_drop_v1(p: *mut dyn SchemaParser) {
    if !p.is_null() {
        // SAFETY: pointer originated from `Box::into_raw` in `tablec_plugin_create_v1`.
        unsafe { drop(Box::from_raw(p)) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_returns_fixture() {
        assert_eq!(FixtureParser.name(), "fixture");
    }

    #[test]
    fn parse_delegates_to_standard() {
        let sheet = vec![
            vec!["id".into(), "name".into()],
            vec!["int".into(), "string".into()],
            vec!["".into(), "".into()],
            vec!["".into(), "".into()],
            vec!["".into(), "".into()],
            vec!["1".into(), "alice".into()],
        ];
        let r = FixtureParser.parse_schema("T", &sheet).unwrap();
        assert!(matches!(r, SchemaParseResult::Schema(_)));
    }
}