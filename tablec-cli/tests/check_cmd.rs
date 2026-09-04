//! Integration tests for `tablec check` — the CLI surface of the shared
//! check pipeline. These run the real binary via `CARGO_BIN_EXE_tablec` and
//! pin the process-level contract: exit codes, the once-only reporting of
//! project diagnostics, and the no-files notice.

use std::process::{Command, Output};

use rust_xlsxwriter::Workbook;

/// Write a sheet using the standard 5-row tablec layout.
fn add_sheet(
    wb: &mut Workbook,
    name: &str,
    columns: &[(&str, &str, &str)], // (name, type, constraint)
    data: &[&[&str]],
) {
    let sheet = wb.add_worksheet();
    sheet.set_name(name).ok();
    for (col, (field_name, ty, constraint)) in columns.iter().enumerate() {
        let col = col as u16;
        sheet.write_string(0, col, *field_name).ok();
        sheet.write_string(1, col, *ty).ok();
        sheet.write_string(2, col, "").ok();
        sheet.write_string(3, col, *constraint).ok();
        sheet.write_string(4, col, "").ok();
    }
    for (row, cells) in data.iter().enumerate() {
        for (col, cell) in cells.iter().enumerate() {
            sheet.write_string(5 + row as u32, col as u16, *cell).ok();
        }
    }
}

fn save(mut wb: Workbook, dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    wb.save(&path).unwrap();
    path
}

/// Target table `Item` (column `id`).
fn write_items(dir: &std::path::Path, name: &str, ids: &[&str]) -> std::path::PathBuf {
    let mut wb = Workbook::new();
    let rows: Vec<&[&str]> = ids.iter().map(std::slice::from_ref).collect();
    add_sheet(&mut wb, "Item", &[("id", "int", "")], &rows);
    save(wb, dir, name)
}

/// Host table `Drop` (column `item_id`) holding `@ref("Item.id")`.
fn write_drop(dir: &std::path::Path, name: &str, item_ids: &[&str]) -> std::path::PathBuf {
    let mut wb = Workbook::new();
    let rows: Vec<&[&str]> = item_ids.iter().map(std::slice::from_ref).collect();
    add_sheet(
        &mut wb,
        "Drop",
        &[("item_id", "int", "@ref(\"Item.id\")")],
        &rows,
    );
    save(wb, dir, name)
}

/// A minimal config so the check target's globs don't depend on whatever
/// `tablec.toml` happens to sit in the test process's cwd.
fn write_config(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("tablec.toml");
    std::fs::write(
        &path,
        "[project]\nname = \"check_cli\"\n\n[data]\ninput_dir = \".\"\n\n[export]\nformat = \"json\"\noutput_dir = \"out\"\n",
    )
    .unwrap();
    path
}

fn run_check(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablec"))
        .args(args)
        .output()
        .expect("spawn tablec binary")
}

#[test]
fn clean_fixture_exits_zero() {
    let dir = tempfile::tempdir().unwrap();
    let config = write_config(dir.path());
    let _items = write_items(dir.path(), "items.xlsx", &["1", "2"]);
    let _drop = write_drop(dir.path(), "drop.xlsx", &["1"]);

    let out = run_check(&[
        "check",
        "--config",
        config.to_str().unwrap(),
        dir.path().to_str().unwrap(),
    ]);
    assert!(
        out.status.success(),
        "expected exit 0, got {:?}\nstdout: {}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Check finished successfully"),
        "expected success summary, got: {stdout}"
    );
    // Per-sheet result lines.
    assert!(stdout.contains("Checking sheet: Item"), "got: {stdout}");
    assert!(stdout.contains("Checking sheet: Drop"), "got: {stdout}");
}

#[test]
fn cross_file_ref_violation_reports_once_and_exits_nonzero() {
    let dir = tempfile::tempdir().unwrap();
    let config = write_config(dir.path());
    // Host value 99 is absent from the target column in the OTHER file.
    let _items = write_items(dir.path(), "b_items.xlsx", &["1"]);
    let _drop = write_drop(dir.path(), "a_drop.xlsx", &["99"]);

    let out = run_check(&[
        "check",
        "--config",
        config.to_str().unwrap(),
        dir.path().to_str().unwrap(),
    ]);
    assert!(
        !out.status.success(),
        "cross-table @ref violation must fail the command"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        combined.matches("ConstraintForeignKeyViolation").count(),
        1,
        "violation must be reported exactly once, got: {combined}"
    );
    assert!(combined.contains("Found 1 errors"), "got: {combined}");
    assert!(
        combined.contains("missing from target Item.id"),
        "expected the FK violation message, got: {combined}"
    );
}

#[test]
fn no_files_fixture_exits_zero() {
    let dir = tempfile::tempdir().unwrap();
    let config = write_config(dir.path());
    std::fs::write(dir.path().join("readme.txt"), "not a spreadsheet").unwrap();

    let out = run_check(&[
        "check",
        "--config",
        config.to_str().unwrap(),
        dir.path().to_str().unwrap(),
    ]);
    assert!(
        out.status.success(),
        "no-files must stay a notice + exit zero, got {:?}",
        out.status.code()
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("No Excel files found to check."),
        "got: {stdout}"
    );
}

#[test]
fn unknown_parser_flag_fails_loudly() {
    // `--parser` must actually be used now (it used to be resolved into a
    // dead store and silently ignored). An unregistered name must surface as
    // a failure instead of a silent all-clear.
    let dir = tempfile::tempdir().unwrap();
    let config = write_config(dir.path());
    let _items = write_items(dir.path(), "items.xlsx", &["1"]);

    let out = run_check(&[
        "check",
        "--config",
        config.to_str().unwrap(),
        "--parser",
        "no-such-parser",
        dir.path().to_str().unwrap(),
    ]);
    assert!(
        !out.status.success(),
        "unknown --parser must fail (parser selection is honored now)"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("parser 'no-such-parser' not registered"),
        "got: {combined}"
    );
}

#[test]
fn single_file_target_is_checked() {
    let dir = tempfile::tempdir().unwrap();
    let config = write_config(dir.path());
    let good = write_items(dir.path(), "good.xlsx", &["1"]);
    let _other = write_drop(dir.path(), "other_drop.xlsx", &["7"]); // target Item missing for this file alone

    // Checking the single good file must not be affected by the other file.
    let out = run_check(&[
        "check",
        "--config",
        config.to_str().unwrap(),
        good.to_str().unwrap(),
    ]);
    assert!(
        out.status.success(),
        "single-file target should check only that file\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Checking sheet: Item"), "got: {stdout}");
    assert!(!stdout.contains("Drop"), "got: {stdout}");
}
