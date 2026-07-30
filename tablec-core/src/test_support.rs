//! 测试辅助模块（公开 API，但仅供测试代码使用）。
//!
//! - `cdylib_fixture::build()`    — 编译 `tests/fixtures/cdylib_parser` cdylib fixture 并返回 .so 路径
//! - `cdylib_fixture::fixture_xlsx()` — 返回已提交的 fixture xlsx（5 行标准布局）
//!
//! 设计意图：把"如何编译并加载 cdylib fixture"的细节集中在一个地方；
//! 集成测试 `tests/cdylib_e2e.rs` 直接调用，不要再 inline。
pub mod cdylib_fixture {
    include!("../tests/fixtures/cdylib_parser/build_and_test.rs");
}
