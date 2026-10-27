//! HTML validation report renderer.
//!
//! Hand-rolled writer with a single dedicated [`html_escape`] helper — no
//! template crate (one fixed-structure artifact does not justify proc-macro
//! compile weight in core, and inline markup stays within reach of the
//! coverage rule). Every dynamic string passes through [`html_escape`]; no
//! diagnostic content ever reaches the output unescaped.
//!
//! The document is fully self-contained: inline CSS, inline vanilla-JS
//! severity filter, zero external requests, so it renders correctly
//! offline. UI labels are Chinese; diagnostic messages stay as produced by
//! core.

use super::Report;
use crate::core::diagnostic::{Diagnostic, Severity};

/// Escape a string for safe interpolation into HTML text content and
/// attribute values. Single-pass: `<>&"'` become entities, the whitespace
/// controls (tab, LF, CR) pass through raw — they cannot form markup — and
/// any other control character becomes a numeric character reference (NUL
/// becomes U+FFFD, which HTML forbids even as a numeric reference). There is
/// intentionally no double-escape detection: already-escaped input is
/// escaped again, which is the correct single-pass behavior.
pub(crate) fn html_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            '\t' | '\n' | '\r' => out.push(c),
            '\0' => out.push('\u{FFFD}'),
            c if c.is_control() => {
                out.push_str("&#");
                out.push_str(&(c as u32).to_string());
                out.push(';');
            }
            c => out.push(c),
        }
    }
    out
}

/// One sheet-level group of diagnostics inside a file group.
struct SheetGroup {
    sheet: Option<String>,
    /// Indices into the report's diagnostics, in pipeline order.
    items: Vec<usize>,
}

/// One file-level group of diagnostics.
struct FileGroup {
    file: Option<String>,
    sheets: Vec<SheetGroup>,
}

/// Group diagnostics by file, then sheet, preserving first-appearance order
/// at both levels (the pipeline order within a sheet is the index order).
fn group_by_file_sheet(diags: &[Diagnostic]) -> Vec<FileGroup> {
    let mut files: Vec<FileGroup> = Vec::new();
    for (i, d) in diags.iter().enumerate() {
        let file = d
            .location
            .file
            .as_ref()
            .map(|f| f.to_string_lossy().into_owned());
        let sheet = d.location.sheet.clone();
        let file_group = match files.iter_mut().find(|g| g.file == file) {
            Some(g) => g,
            None => {
                files.push(FileGroup {
                    file,
                    sheets: Vec::new(),
                });
                files.last_mut().expect("just pushed")
            }
        };
        match file_group.sheets.iter_mut().find(|g| g.sheet == sheet) {
            Some(s) => s.items.push(i),
            None => file_group.sheets.push(SheetGroup {
                sheet,
                items: vec![i],
            }),
        }
    }
    files
}

/// Placeholder heading for a diagnostic without a file/sheet location.
const NO_FILE_LABEL: &str = "（未知文件）";
const NO_SHEET_LABEL: &str = "（未知工作表）";

impl Report {
    /// Render the report as a self-contained single HTML file: all styles
    /// and scripts inline, no external requests, renders correctly offline.
    pub fn render_html(&self) -> String {
        let mut out = String::with_capacity(8192);
        out.push_str(HTML_HEAD);
        out.push_str("<main>\n<h1>tablec 校验报告</h1>\n");

        // Summary card: severity counts, file/sheet census.
        out.push_str("<section class=\"summary\">\n");
        out.push_str(&format!(
            "<div class=\"stat stat-errors\"><span class=\"num\">{}</span><span class=\"label\">错误</span></div>\n",
            self.summary.errors
        ));
        out.push_str(&format!(
            "<div class=\"stat stat-warnings\"><span class=\"num\">{}</span><span class=\"label\">警告</span></div>\n",
            self.summary.warnings
        ));
        out.push_str(&format!(
            "<div class=\"stat\"><span class=\"num\">{}</span><span class=\"label\">检查文件</span></div>\n",
            self.summary.files_checked
        ));
        out.push_str(&format!(
            "<div class=\"stat\"><span class=\"num\">{}</span><span class=\"label\">检查表</span></div>\n",
            self.summary.tables_checked
        ));
        out.push_str("</section>\n");

        // Run metadata.
        out.push_str("<dl class=\"meta\">\n");
        out.push_str(&format!(
            "<div class=\"meta-row\"><dt>生成时间</dt><dd>{}</dd></div>\n",
            html_escape(&self.generated_at)
        ));
        out.push_str(&format!(
            "<div class=\"meta-row\"><dt>工具版本</dt><dd>{}</dd></div>\n",
            html_escape(&self.tablec_version)
        ));
        out.push_str(&format!(
            "<div class=\"meta-row\"><dt>解析器</dt><dd>{}</dd></div>\n",
            html_escape(&self.parser)
        ));
        out.push_str(&format!(
            "<div class=\"meta-row\"><dt>输入目录</dt><dd>{}</dd></div>\n",
            html_escape(&self.input_dir)
        ));
        out.push_str("</dl>\n");

        if self.diagnostics.is_empty() {
            // Distinct clean-run empty state: no issues section at all.
            out.push_str("<section class=\"empty-state\">未发现问题，全部通过。</section>\n");
        } else {
            // Severity filter (inline vanilla JS at the end of the document).
            out.push_str(
                "<section class=\"filter\" aria-label=\"按严重级别筛选\">\
<button type=\"button\" data-filter=\"all\" class=\"active\">全部</button>\
<button type=\"button\" data-filter=\"error\">仅错误</button>\
<button type=\"button\" data-filter=\"warning\">仅警告</button>\
</section>\n",
            );

            out.push_str("<section class=\"issues\">\n<h2>问题明细</h2>\n");
            for file_group in group_by_file_sheet(&self.diagnostics) {
                let file_label = match &file_group.file {
                    Some(f) => html_escape(f),
                    None => NO_FILE_LABEL.to_string(),
                };
                out.push_str(&format!(
                    "<div class=\"file-group\">\n<h3 class=\"file\">{file_label}</h3>\n"
                ));
                for sheet_group in &file_group.sheets {
                    let sheet_label = match &sheet_group.sheet {
                        Some(s) => html_escape(s),
                        None => NO_SHEET_LABEL.to_string(),
                    };
                    out.push_str(&format!(
                        "<div class=\"sheet-group\">\n<h4 class=\"sheet\">{sheet_label}</h4>\n"
                    ));
                    for &i in &sheet_group.items {
                        render_diag(&mut out, &self.diagnostics[i]);
                    }
                    out.push_str("</div>\n");
                }
                out.push_str("</div>\n");
            }
            out.push_str("</section>\n");
        }

        // Passed sheets.
        out.push_str("<section class=\"passed\">\n<h2>通过的表</h2>\n");
        if self.passed_sheets.is_empty() {
            out.push_str("<p class=\"passed-empty\">没有通过的表。</p>\n");
        } else {
            out.push_str("<ul class=\"passed-list\">\n");
            for name in &self.passed_sheets {
                out.push_str(&format!("<li>{}</li>\n", html_escape(name)));
            }
            out.push_str("</ul>\n");
        }
        out.push_str("</section>\n");

        out.push_str("</main>\n");
        out.push_str(HTML_SCRIPT);
        out.push_str("</body>\n</html>\n");
        out
    }
}

/// Render one diagnostic entry: severity badge, code, location, message.
fn render_diag(out: &mut String, d: &Diagnostic) {
    let (sev_name, sev_label) = match d.severity {
        Severity::Error => ("error", "错误"),
        Severity::Warning => ("warning", "警告"),
    };
    out.push_str(&format!(
        "<div class=\"diag\" data-severity=\"{sev_name}\">\
<span class=\"badge badge-{sev_name}\">{sev_label}</span>\
<span class=\"code\">{}</span>",
        html_escape(&format!("{:?}", d.code))
    ));

    // Source location: sheet and line/column, each part when present.
    let mut loc = String::new();
    if let Some(sheet) = d.location.sheet.as_deref() {
        loc.push_str(&html_escape(sheet));
    }
    if let (Some(line), Some(column)) = (d.location.line, d.location.column) {
        if !loc.is_empty() {
            loc.push(' ');
        }
        loc.push_str(&format!("{line}:{column}"));
    }
    if !loc.is_empty() {
        out.push_str(&format!("<span class=\"loc\">位置：{loc}</span>"));
    }

    out.push_str(&format!(
        "<p class=\"message\">{}</p>\n</div>\n",
        html_escape(&d.message)
    ));
}

const HTML_HEAD: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>tablec 校验报告</title>
<style>
:root { color-scheme: light; }
* { box-sizing: border-box; }
body { font-family: -apple-system, "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif; margin: 0; background: #f5f6f8; color: #1f2328; }
main { max-width: 960px; margin: 0 auto; padding: 24px 16px 48px; }
h1 { font-size: 22px; }
h2 { font-size: 17px; margin: 24px 0 8px; }
.summary { display: flex; flex-wrap: wrap; gap: 12px; margin: 16px 0; }
.stat { background: #fff; border: 1px solid #e1e4e8; border-radius: 8px; padding: 12px 20px; min-width: 96px; }
.stat .num { display: block; font-size: 24px; font-weight: 600; }
.stat .label { color: #57606a; font-size: 12px; }
.stat-errors .num { color: #cf222e; }
.stat-warnings .num { color: #9a6700; }
.meta { background: #fff; border: 1px solid #e1e4e8; border-radius: 8px; padding: 12px 16px; margin: 16px 0; }
.meta-row { display: flex; margin: 2px 0; font-size: 13px; }
.meta-row dt { color: #57606a; min-width: 72px; }
.meta-row dd { margin: 0; word-break: break-all; }
.filter { margin: 16px 0; }
.filter button { border: 1px solid #d0d7de; background: #fff; border-radius: 6px; padding: 4px 14px; margin-right: 8px; cursor: pointer; font-size: 13px; }
.filter button.active { background: #0969da; border-color: #0969da; color: #fff; }
.file-group, .sheet-group { margin: 8px 0; }
h3.file { margin: 14px 0 4px; font-size: 15px; font-family: SFMono-Regular, Consolas, monospace; }
h4.sheet { margin: 8px 0 4px; color: #57606a; font-weight: 500; font-size: 14px; }
.diag { background: #fff; border: 1px solid #e1e4e8; border-left-width: 4px; border-radius: 6px; padding: 8px 12px; margin: 8px 0; }
.diag[data-severity="error"] { border-left-color: #cf222e; }
.diag[data-severity="warning"] { border-left-color: #d4a72c; }
.badge { display: inline-block; font-size: 12px; border-radius: 999px; padding: 1px 10px; margin-right: 8px; color: #fff; }
.badge-error { background: #cf222e; }
.badge-warning { background: #9a6700; }
.code { font-family: SFMono-Regular, Consolas, monospace; font-size: 12px; color: #57606a; margin-right: 8px; }
.loc { font-size: 12px; color: #57606a; }
.message { margin: 6px 0 0; font-size: 13px; white-space: pre-wrap; }
.passed ul { list-style: none; padding: 0; display: flex; flex-wrap: wrap; gap: 8px; }
.passed li { background: #fff; border: 1px solid #e1e4e8; border-radius: 6px; padding: 4px 12px; font-size: 13px; }
.empty-state { background: #dafbe1; border: 1px solid #4ac26b; color: #116329; border-radius: 8px; padding: 16px; margin: 16px 0; font-size: 14px; }
</style>
</head>
<body>
"#;

const HTML_SCRIPT: &str = r#"<script>
(function () {
  var buttons = document.querySelectorAll(".filter button");
  buttons.forEach(function (btn) {
    btn.addEventListener("click", function () {
      buttons.forEach(function (b) { b.classList.remove("active"); });
      btn.classList.add("active");
      var f = btn.getAttribute("data-filter");
      document.querySelectorAll(".diag").forEach(function (d) {
        d.hidden = f !== "all" && d.getAttribute("data-severity") !== f;
      });
    });
  });
})();
</script>
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::check::CheckOutcome;
    use crate::core::diagnostic::{DiagnosticCode, SourceLocation};
    use std::path::PathBuf;

    fn table(name: &str) -> crate::core::table::table::Table {
        crate::core::table::table::Table {
            name: name.to_string(),
            schema: crate::core::schema::Schema::from_parts(vec![], vec![]),
            data: vec![],
        }
    }

    fn diag(
        sev: Severity,
        msg: &str,
        file: Option<&str>,
        sheet: Option<&str>,
        line: Option<u32>,
        column: Option<u32>,
    ) -> Diagnostic {
        Diagnostic {
            severity: sev,
            code: DiagnosticCode::ValueOutOfRange,
            message: msg.to_string(),
            location: SourceLocation {
                file: file.map(PathBuf::from),
                sheet: sheet.map(str::to_string),
                line,
                column,
            },
        }
    }

    fn report(tables: &[&str], diags: Vec<Diagnostic>) -> Report {
        let outcome = CheckOutcome {
            tables: tables.iter().map(|n| table(n)).collect(),
            diagnostics: diags,
            files: vec![std::path::PathBuf::from("tables.xlsx")],
        };
        Report::from_outcome_at(&outcome, "0.1.0", "tables/", "standard", 0)
    }

    /// Every `src=` / `href=` / `url(` occurrence must not be followed by an
    /// `http` / `https` / protocol-relative (`//`) reference.
    fn external_ref_violations(html: &str) -> Vec<String> {
        let mut bad = Vec::new();
        for marker in ["src=", "href=", "url("] {
            let mut search = 0;
            while let Some(pos) = html[search..].find(marker) {
                let after = &html[search + pos + marker.len()..];
                let trimmed = after.trim_start();
                let unquoted = trimmed
                    .strip_prefix('"')
                    .or_else(|| trimmed.strip_prefix('\''))
                    .unwrap_or(trimmed);
                if unquoted.starts_with("http") || unquoted.starts_with("//") {
                    bad.push(format!("{marker}{}", &unquoted[..unquoted.len().min(20)]));
                }
                search += pos + marker.len();
            }
        }
        bad
    }

    // -------------------------------------------------------------------
    // 1.4 html_escape
    // -------------------------------------------------------------------

    #[test]
    fn escape_handles_markup_delimiters_and_quotes() {
        assert_eq!(html_escape("<>&\"'"), "&lt;&gt;&amp;&quot;&#39;");
    }

    #[test]
    fn escape_leaves_plain_text_and_non_ascii_untouched() {
        assert_eq!(html_escape("plain text 123"), "plain text 123");
        assert_eq!(html_escape("中文消息"), "中文消息");
    }

    #[test]
    fn escape_renders_control_characters_inert() {
        // Tab, LF, CR pass through raw (cannot form markup).
        assert_eq!(html_escape("a\tb\nc\rd"), "a\tb\nc\rd");
        // Other C0 controls become numeric references; NUL becomes U+FFFD
        // (HTML forbids a NUL reference).
        assert_eq!(html_escape("a\u{1}b"), "a&#1;b");
        assert_eq!(html_escape("a\u{7}b"), "a&#7;b");
        assert_eq!(html_escape("a\u{0}b"), "a\u{FFFD}b");
    }

    #[test]
    fn escape_single_pass_no_double_escape_of_escaped_looking_input() {
        // Already-escaped-looking input is escaped again — correct
        // single-pass output, no double-escape detection.
        assert_eq!(html_escape("&amp;"), "&amp;amp;");
        assert_eq!(html_escape("&lt;tag&gt;"), "&amp;lt;tag&amp;gt;");
        assert_eq!(html_escape("&#39;"), "&amp;#39;");
    }

    #[test]
    fn escape_empty_and_capacity_edge() {
        assert_eq!(html_escape(""), "");
        // A string of only escapable chars must not truncate.
        assert_eq!(html_escape("&&&&"), "&amp;&amp;&amp;&amp;");
    }

    // -------------------------------------------------------------------
    // 1.5 HTML document requirements
    // -------------------------------------------------------------------

    #[test]
    fn html_is_self_contained_no_external_references() {
        let r = report(
            &["Item"],
            vec![
                diag(
                    Severity::Error,
                    "boom",
                    Some("a.xlsx"),
                    Some("Item"),
                    Some(6),
                    Some(2),
                ),
                diag(
                    Severity::Warning,
                    "care",
                    Some("b.xlsx"),
                    Some("Drop"),
                    None,
                    None,
                ),
            ],
        );
        let html = r.render_html();
        assert!(
            html.starts_with("<!DOCTYPE html>"),
            "must be a complete document"
        );
        assert!(!html.contains("<link"), "no link tags allowed, got: {html}");
        assert!(
            !html.contains("<script src") && !html.contains("<img") && !html.contains("<iframe"),
            "no external-resource elements allowed"
        );
        let violations = external_ref_violations(&html);
        assert!(
            violations.is_empty(),
            "found external references: {violations:?}"
        );
        // Styles and script are inline.
        assert!(html.contains("<style>"));
        assert!(html.contains("<script>"));
    }

    #[test]
    fn html_self_contained_for_clean_run_too() {
        let html = report(&["Item"], vec![]).render_html();
        assert!(external_ref_violations(&html).is_empty());
    }

    #[test]
    fn ref_scan_flags_http_https_and_protocol_relative_references() {
        let flagged = r#"<script src="http://evil/x.js"></script>
<a href='https://evil/page'>x</a>
<style>body { background: url(//evil/pic.png); }</style>"#;
        let hits = external_ref_violations(flagged);
        assert_eq!(hits.len(), 3, "http, https and // must all be flagged");
        // Escaped content is inert text, not a live reference.
        let inert = r#"<p>src=&quot;http://evil&quot;</p>"#;
        assert!(external_ref_violations(inert).is_empty());
    }

    #[test]
    fn html_groups_by_file_then_sheet_in_first_appearance_order() {
        let r = report(
            &[],
            vec![
                diag(
                    Severity::Error,
                    "m1",
                    Some("a.xlsx"),
                    Some("S1"),
                    None,
                    None,
                ),
                diag(
                    Severity::Warning,
                    "m2",
                    Some("b.xlsx"),
                    Some("S9"),
                    None,
                    None,
                ),
                diag(
                    Severity::Error,
                    "m3",
                    Some("a.xlsx"),
                    Some("S2"),
                    None,
                    None,
                ),
                diag(
                    Severity::Warning,
                    "m4",
                    Some("a.xlsx"),
                    Some("S1"),
                    None,
                    None,
                ),
            ],
        );
        let html = r.render_html();
        let pos = |needle: &str| html.find(needle).expect(needle);
        // File groups in first-appearance order: a.xlsx before b.xlsx.
        assert!(pos("<h3 class=\"file\">a.xlsx</h3>") < pos("<h3 class=\"file\">b.xlsx</h3>"));
        // Sheet groups within a.xlsx in first-appearance order: S1 before S2
        // (b.xlsx's S9 comes in between as its own group).
        let a_group_start = pos("<h3 class=\"file\">a.xlsx</h3>");
        let b_group_start = pos("<h3 class=\"file\">b.xlsx</h3>");
        let s1 = pos("<h4 class=\"sheet\">S1</h4>");
        let s2 = pos("<h4 class=\"sheet\">S2</h4>");
        assert!(a_group_start < s1 && s1 < s2 && s2 < b_group_start);
        // Entries inside a group keep pipeline order.
        let s1_heading = pos("<h4 class=\"sheet\">S1</h4>");
        let m1 = pos("m1");
        let m4 = pos("m4");
        assert!(s1_heading < m1 && m1 < m4);
    }

    #[test]
    fn html_groups_locationless_diagnostics_under_placeholders() {
        // The pipeline's no-files warning carries no file/sheet location.
        let r = report(
            &[],
            vec![diag(Severity::Warning, "no files", None, None, None, None)],
        );
        let html = r.render_html();
        assert!(html.contains(NO_FILE_LABEL));
        assert!(html.contains(NO_SHEET_LABEL));
    }

    #[test]
    fn html_severity_badges_use_distinct_classes() {
        let r = report(
            &[],
            vec![
                diag(
                    Severity::Error,
                    "boom",
                    Some("a.xlsx"),
                    Some("S"),
                    None,
                    None,
                ),
                diag(
                    Severity::Warning,
                    "care",
                    Some("a.xlsx"),
                    Some("S"),
                    None,
                    None,
                ),
            ],
        );
        let html = r.render_html();
        assert!(html.contains("<span class=\"badge badge-error\">错误</span>"));
        assert!(html.contains("<span class=\"badge badge-warning\">警告</span>"));
        assert!(html.contains("<div class=\"diag\" data-severity=\"error\">"));
        assert!(html.contains("<div class=\"diag\" data-severity=\"warning\">"));
    }

    #[test]
    fn html_entries_show_code_location_and_message() {
        let r = report(
            &[],
            vec![diag(
                Severity::Error,
                "200 not in int8 range",
                Some("a.xlsx"),
                Some("Item"),
                Some(6),
                Some(2),
            )],
        );
        let html = r.render_html();
        assert!(html.contains("ValueOutOfRange"), "code shown");
        assert!(
            html.contains("位置：Item 6:2"),
            "sheet and line/column shown"
        );
        assert!(html.contains("200 not in int8 range"), "message shown");
    }

    #[test]
    fn html_escapes_diagnostic_content_end_to_end() {
        let r = report(
            &[],
            vec![diag(
                Severity::Error,
                "<img src=x onerror=alert(1)> & \"quotes\"",
                Some("a<b>.xlsx"),
                Some("S&S"),
                None,
                None,
            )],
        );
        let html = r.render_html();
        assert!(
            html.contains("&lt;img src=x onerror=alert(1)&gt; &amp; &quot;quotes&quot;"),
            "message must be escaped, got: {html}"
        );
        assert!(html.contains("a&lt;b&gt;.xlsx"), "file heading escaped");
        assert!(html.contains("S&amp;S"), "sheet heading escaped");
        assert!(!html.contains("<img"), "no raw markup from content");
        // And the escaping did not introduce external references.
        assert!(external_ref_violations(&html).is_empty());
    }

    #[test]
    fn html_summary_card_carries_counts_and_metadata() {
        let r = report(
            &["Item", "Drop"],
            vec![
                diag(
                    Severity::Error,
                    "e",
                    Some("a.xlsx"),
                    Some("Item"),
                    None,
                    None,
                ),
                diag(
                    Severity::Warning,
                    "w",
                    Some("a.xlsx"),
                    Some("Item"),
                    None,
                    None,
                ),
            ],
        );
        let html = r.render_html();
        for label in ["错误", "警告", "检查文件", "检查表"] {
            assert!(html.contains(label), "summary card label {label}");
        }
        // Metadata: generation time (epoch 0), tool version, parser, dir.
        assert!(html.contains("生成时间"));
        assert!(html.contains("1970-01-01T00:00:00Z"));
        assert!(html.contains("工具版本"));
        assert!(html.contains("0.1.0"));
        assert!(html.contains("解析器"));
        assert!(html.contains("standard"));
        assert!(html.contains("输入目录"));
        assert!(html.contains("tables/"));
    }

    #[test]
    fn html_ui_labels_are_chinese() {
        // A report with diagnostics: the severity filter only renders when
        // there is something to filter.
        let r = report(
            &["Item"],
            vec![diag(
                Severity::Error,
                "e",
                Some("a.xlsx"),
                Some("S"),
                None,
                None,
            )],
        );
        let html = r.render_html();
        for label in [
            "校验报告", // title
            "全部",     // filter: all
            "仅错误",   // filter: errors only
            "仅警告",   // filter: warnings only
            "问题明细", // issues heading
            "通过的表", // passed sheets heading
            "生成时间", // generated at
            "工具版本", // tool version
            "解析器",   // parser
            "输入目录", // input directory
        ] {
            assert!(html.contains(label), "missing Chinese UI label: {label}");
        }
    }

    #[test]
    fn html_clean_run_has_distinct_empty_state() {
        let r = report(&["Item", "Drop"], vec![]);
        let html = r.render_html();
        assert!(html.contains("未发现问题，全部通过。"), "empty state shown");
        assert!(
            !html.contains("问题明细"),
            "clean run has no issues section"
        );
        assert!(!html.contains("仅错误"), "clean run has no severity filter");
        // Passed sheets are still listed.
        assert!(html.contains("<li>Item</li>"));
        assert!(html.contains("<li>Drop</li>"));
    }

    #[test]
    fn html_failed_run_has_no_empty_state() {
        let r = report(
            &[],
            vec![diag(
                Severity::Error,
                "e",
                Some("a.xlsx"),
                Some("S"),
                None,
                None,
            )],
        );
        let html = r.render_html();
        assert!(!html.contains("未发现问题"));
        assert!(html.contains("问题明细"));
    }

    #[test]
    fn html_passed_sheets_exclude_sheets_with_findings() {
        // Drop carries a diagnostic; Item is passed.
        let r = report(
            &["Item", "Drop"],
            vec![diag(
                Severity::Error,
                "e",
                Some("a.xlsx"),
                Some("Drop"),
                None,
                None,
            )],
        );
        let html = r.render_html();
        assert!(html.contains("<li>Item</li>"), "clean sheet listed");
        assert!(!html.contains("<li>Drop</li>"), "failing sheet not listed");
        assert!(html.contains("通过的表"));
    }

    #[test]
    fn html_passed_list_escapes_sheet_names() {
        let outcome = CheckOutcome {
            tables: vec![table("<b>Evil</b>")],
            diagnostics: vec![],
            files: vec![],
        };
        let r = Report::from_outcome_at(&outcome, "v", "d", "p", 0);
        let html = r.render_html();
        assert!(html.contains("<li>&lt;b&gt;Evil&lt;/b&gt;</li>"));
        assert!(!html.contains("<li><b>"));
    }

    #[test]
    fn html_no_passed_sheets_has_empty_marker() {
        let r = report(
            &[],
            vec![diag(Severity::Error, "e", Some("a"), Some("S"), None, None)],
        );
        let html = r.render_html();
        assert!(html.contains("没有通过的表。"));
    }

    #[test]
    fn html_filter_script_targets_severity_data_attributes() {
        let r = report(
            &[],
            vec![diag(Severity::Error, "e", Some("a"), Some("S"), None, None)],
        );
        let html = r.render_html();
        // The inline filter narrows entries by their data-severity value.
        assert!(html.contains("data-filter=\"all\""));
        assert!(html.contains("data-filter=\"error\""));
        assert!(html.contains("data-filter=\"warning\""));
        assert!(html.contains("getAttribute(\"data-severity\")"));
    }
}
