//! 端到端：编译 cdylib fixture → 动态加载 → 用加载的 parser 解析 fixture xlsx
//!
//! 这两个测试都被标记 `#[ignore]`，原因：
//! - 它们会 spawn `cargo build`，耗时可达数十秒；CI 默认跑 `cargo test` 不应被拖慢
//! - 需要目标主机与 fixture crate 用同一 Rust 工具链编译
//!
//! 跑全部：
//!     cargo test --test cdylib_e2e -- --ignored --nocapture
//! 只跑单个：
//!     cargo test --test cdylib_e2e -- --ignored --nocapture load_fixture_cdylib_and_inspect_name
use std::path::Path;

use tablec_core::core::schema::SchemaParser;
use tablec_core::core::schema::dynamic::DynamicPlugin;
use tablec_core::core::table::table::read_excel_with;
use tablec_core::test_support::cdylib_fixture;

/// 编译 fixture crate → load → 调用 `name()`：验证符号导出 + vtable dispatch。
#[test]
#[ignore]
fn load_fixture_cdylib_and_inspect_name() {
    let so = cdylib_fixture::build();
    assert!(so.exists(), "fixture .so missing: {:?}", so);
    eprintln!("loaded: {}", so.display());

    // SAFETY: fixture is built from the same Rust toolchain as the host; this is
    // exactly the contract documented on `DynamicPlugin::load`.
    let plugin = unsafe { DynamicPlugin::load(Path::new(&so)) }.expect("load cdylib");
    assert_eq!(plugin.name(), "fixture");
}

/// 编译 fixture crate → load → 解析 fixture xlsx：验证插件在真实 Excel 解析链路上工作。
#[test]
#[ignore]
fn load_fixture_cdylib_and_parse_xlsx() {
    let so = cdylib_fixture::build();
    let xlsx = cdylib_fixture::fixture_xlsx();
    assert!(xlsx.exists(), "fixture xlsx missing: {:?}", xlsx);

    // SAFETY: 同上 — fixture 与 host 用相同 Rust 工具链编译。
    let plugin = unsafe { DynamicPlugin::load(Path::new(&so)) }.expect("load cdylib");
    assert_eq!(plugin.name(), "fixture");

    let tables = read_excel_with(xlsx.to_str().unwrap(), &*plugin)
        .expect("read_excel_with via DynamicPlugin should succeed");
    assert!(
        !tables.is_empty(),
        "expected at least one table from fixture xlsx"
    );

    // 至少一张表名匹配 fixture sheet 名 `items`
    let names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
    assert!(
        names.contains(&"items"),
        "expected sheet `items` among parsed tables, got {:?}",
        names
    );

    // 第一张表至少有一行数据
    let first = &tables[0];
    assert!(
        !first.data.is_empty(),
        "expected rows in table `{}`",
        first.name
    );
}

/// 加载不存在的 .so → DynamicPluginError::Load（不依赖 fixture crate）
#[test]
fn load_nonexistent_so_yields_load_error() {
    let r = unsafe {
        DynamicPlugin::load(Path::new(
            "/tmp/tablec_no_such_plugin_definitely_missing.so",
        ))
    };
    assert!(r.is_err(), "expected Err for missing .so, got Ok");
}
