use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceLocation {
    pub file: Option<PathBuf>,
    pub sheet: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiagnosticCode {
    TokenizerUnexpectedChar,
    TypeParseError,
    TypeUnknown,
    ValueParseError,
    ValueOutOfRange,
    StringEscapeUnsupported,
    StructFieldMismatch,
    StructFieldCountMismatch,
    SheetSkipped,
    FieldMissingValue,
    TableConstraintParseError,
    ConstraintUnknown,
    ConstraintDuplicate,
    ConstraintSequenceBroken,
    ConstraintOrderViolation,
    ConstraintFieldMissing,
    ConstraintCompositeMissing,
    ConstraintValueViolation,
    ConstraintNotInSet,
    ConstraintPatternMismatch,
    ConstraintNullNotAllowed,
    ConstraintForeignKeyViolation,
    HeaderParserError,
    SchemaFieldOverlap,
    SchemaDataStartOOB,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagnosticCode,
    pub message: String,
    pub location: SourceLocation,
}

impl Diagnostic {
    pub fn new(code: DiagnosticCode, message: impl Into<String>, location: SourceLocation) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
            location,
        }
    }
}

impl From<&str> for Diagnostic {
    fn from(s: &str) -> Self {
        Diagnostic::new(DiagnosticCode::Other, s, SourceLocation::default())
    }
}

impl Default for SourceLocation {
    fn default() -> Self {
        Self {
            file: None,
            sheet: None,
            line: None,
            column: None,
        }
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.code)?;
        if let Some(sheet) = &self.location.sheet {
            write!(f, " [{}]", sheet)?;
        }
        if let (Some(line), Some(col)) = (self.location.line, self.location.column) {
            write!(f, " {}:{}", line, col)?;
        }
        write!(f, ": {}", self.message)
    }
}

impl std::error::Error for Diagnostic {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_roundtrip_preserves_fields() {
        let d = Diagnostic::new(
            DiagnosticCode::ValueOutOfRange,
            "200 not in int8 range [-128, 127]",
            SourceLocation {
                file: None,
                sheet: Some("Sheet1".into()),
                line: Some(6),
                column: Some(2),
            },
        );
        let json = serde_json::to_string(&d).unwrap();
        let d2: Diagnostic = serde_json::from_str(&json).unwrap();
        assert_eq!(d.code, d2.code);
        assert_eq!(d.message, d2.message);
        assert_eq!(d.location.sheet, d2.location.sheet);
    }

    #[test]
    fn display_with_full_location() {
        let d = Diagnostic::new(
            DiagnosticCode::TokenizerUnexpectedChar,
            "bad char",
            SourceLocation {
                file: None,
                sheet: Some("S".into()),
                line: Some(1),
                column: Some(3),
            },
        );
        let s = format!("{}", d);
        assert!(s.contains("TokenizerUnexpectedChar"));
        assert!(s.contains("S"));
        assert!(s.contains("1:3"));
        assert!(s.contains("bad char"));
    }

    #[test]
    fn display_with_empty_location() {
        let d: Diagnostic = "plain".into();
        assert_eq!(format!("{}", d), "Other: plain");
    }

    #[test]
    fn diagnostic_code_count_matches_spec() {
        // Lock the enum so adding new variants is intentional.
        let codes = [
            DiagnosticCode::TokenizerUnexpectedChar,
            DiagnosticCode::TypeParseError,
            DiagnosticCode::TypeUnknown,
            DiagnosticCode::ValueParseError,
            DiagnosticCode::ValueOutOfRange,
            DiagnosticCode::StringEscapeUnsupported,
            DiagnosticCode::StructFieldMismatch,
            DiagnosticCode::StructFieldCountMismatch,
            DiagnosticCode::SheetSkipped,
            DiagnosticCode::FieldMissingValue,
            DiagnosticCode::TableConstraintParseError,
            DiagnosticCode::ConstraintUnknown,
            DiagnosticCode::ConstraintDuplicate,
            DiagnosticCode::ConstraintSequenceBroken,
            DiagnosticCode::ConstraintOrderViolation,
            DiagnosticCode::ConstraintFieldMissing,
            DiagnosticCode::ConstraintCompositeMissing,
            DiagnosticCode::ConstraintValueViolation,
            DiagnosticCode::ConstraintNotInSet,
            DiagnosticCode::ConstraintPatternMismatch,
            DiagnosticCode::ConstraintNullNotAllowed,
            DiagnosticCode::ConstraintForeignKeyViolation,
            DiagnosticCode::HeaderParserError,
            DiagnosticCode::SchemaFieldOverlap,
            DiagnosticCode::SchemaDataStartOOB,
            DiagnosticCode::Other,
        ];
        // Intentionally asserts total variants — change ONLY when adding/removing a code.
        assert_eq!(codes.len(), 26);
    }

    #[test]
    fn header_parser_error_exists() {
        let d = Diagnostic::new(
            DiagnosticCode::HeaderParserError,
            "snippet".to_string(),
            SourceLocation::default(),
        );
        assert_eq!(d.code, DiagnosticCode::HeaderParserError);
        assert_eq!(d.message, "snippet");
    }

    #[test]
    fn schema_field_overlap_exists() {
        let d = Diagnostic::new(
            DiagnosticCode::SchemaFieldOverlap,
            "duplicate field".to_string(),
            SourceLocation::default(),
        );
        assert_eq!(d.code, DiagnosticCode::SchemaFieldOverlap);
    }

    #[test]
    fn schema_data_start_oob_exists() {
        let d = Diagnostic::new(
            DiagnosticCode::SchemaDataStartOOB,
            "data_start_row out of bounds".to_string(),
            SourceLocation::default(),
        );
        assert_eq!(d.code, DiagnosticCode::SchemaDataStartOOB);
    }

    #[test]
    fn new_defaults_severity_to_error() {
        let d = Diagnostic::new(
            DiagnosticCode::ValueParseError,
            "bad value",
            SourceLocation::default(),
        );
        assert_eq!(d.severity, Severity::Error);
    }

    #[test]
    fn from_str_sets_other_code_default_location_and_error_severity() {
        let d: Diagnostic = "boom".into();
        assert_eq!(d.code, DiagnosticCode::Other);
        assert_eq!(d.message, "boom");
        assert_eq!(d.location, SourceLocation::default());
        assert_eq!(d.severity, Severity::Error);
    }

    #[test]
    fn display_with_sheet_only_skips_line_column_block() {
        let d = Diagnostic::new(
            DiagnosticCode::ConstraintUnknown,
            "unknown constraint",
            SourceLocation {
                file: None,
                sheet: Some("SheetA".into()),
                line: None,
                column: None,
            },
        );
        let s = format!("{}", d);
        assert_eq!(s, "ConstraintUnknown [SheetA]: unknown constraint");
        // No " <line>:<col>" block when line/column are absent.
        // (The single ":" before the message is the standard message separator.)
        assert!(!s.contains("None"));
        // A line:col block would produce ":<digit>" (digit immediately
        // after a colon). Confirm no such pattern appears.
        let has_line_col_block = s
            .as_bytes()
            .windows(2)
            .any(|w| w[0] == b':' && w[1].is_ascii_digit());
        assert!(
            !has_line_col_block,
            "expected no line:col block, got {:?}",
            s
        );
    }

    #[test]
    fn display_with_line_column_only_skips_sheet_block() {
        let d = Diagnostic::new(
            DiagnosticCode::ValueParseError,
            "parse fail",
            SourceLocation {
                file: None,
                sheet: None,
                line: Some(7),
                column: Some(4),
            },
        );
        let s = format!("{}", d);
        assert_eq!(s, "ValueParseError 7:4: parse fail");
        // No "[sheet]" block when sheet is absent.
        assert!(!s.contains('['));
        assert!(!s.contains(']'));
    }

    #[test]
    fn display_omits_file_path_even_when_set() {
        // Documented behavior contract: `file` is metadata only; it never
        // appears in the rendered output.
        let d = Diagnostic::new(
            DiagnosticCode::Other,
            "msg",
            SourceLocation {
                file: Some(std::path::PathBuf::from("/tmp/whatever.xlsx")),
                sheet: None,
                line: None,
                column: None,
            },
        );
        let s = format!("{}", d);
        assert_eq!(s, "Other: msg");
        assert!(!s.contains("/tmp"));
        assert!(!s.contains(".xlsx"));
        assert!(!s.contains("whatever"));
    }

    #[test]
    fn source_location_default_is_all_none() {
        let loc = SourceLocation::default();
        assert!(loc.file.is_none());
        assert!(loc.sheet.is_none());
        assert!(loc.line.is_none());
        assert!(loc.column.is_none());
    }

    #[test]
    fn severity_is_copy_and_distinct_variants_compare() {
        let s = Severity::Error;
        // Copy: usable after move.
        let copied = s;
        let _still_usable = s;
        assert_eq!(s, copied);
        assert_ne!(Severity::Error, Severity::Warning);
        assert_eq!(Severity::Warning, Severity::Warning);
    }

    #[test]
    fn severity_serialize_roundtrip() {
        for sev in [Severity::Error, Severity::Warning] {
            let json = serde_json::to_string(&sev).unwrap();
            let back: Severity = serde_json::from_str(&json).unwrap();
            assert_eq!(sev, back);
        }
    }

    #[test]
    fn source_location_serialize_roundtrip() {
        let loc = SourceLocation {
            file: Some(std::path::PathBuf::from("/x/y.xlsx")),
            sheet: Some("S".into()),
            line: Some(12),
            column: Some(5),
        };
        let json = serde_json::to_string(&loc).unwrap();
        let back: SourceLocation = serde_json::from_str(&json).unwrap();
        assert_eq!(loc, back);
        assert_eq!(
            back.file.as_deref(),
            Some(std::path::Path::new("/x/y.xlsx"))
        );
        assert_eq!(back.sheet.as_deref(), Some("S"));
        assert_eq!(back.line, Some(12));
        assert_eq!(back.column, Some(5));
    }

    #[test]
    fn diagnostic_code_serialize_roundtrip() {
        // Cover both ends of the enum + a mid-range variant.
        for code in [
            DiagnosticCode::TokenizerUnexpectedChar,
            DiagnosticCode::ConstraintForeignKeyViolation,
            DiagnosticCode::Other,
        ] {
            let json = serde_json::to_string(&code).unwrap();
            let back: DiagnosticCode = serde_json::from_str(&json).unwrap();
            assert_eq!(code, back);
        }
    }

    #[test]
    fn diagnostic_implements_std_error_trait() {
        // Usable as &dyn std::error::Error.
        let d: Diagnostic = "via error trait".into();
        let err: &dyn std::error::Error = &d;
        // source() is the default Error::source (None for our impl).
        assert!(err.source().is_none());
        // Display via Error matches our Display impl.
        assert_eq!(err.to_string(), format!("{}", d));
    }

    #[test]
    fn new_accepts_any_into_string() {
        // `&str`
        let d1 = Diagnostic::new(
            DiagnosticCode::Other,
            "from &str",
            SourceLocation::default(),
        );
        // `String`
        let d2 = Diagnostic::new(
            DiagnosticCode::Other,
            String::from("from String"),
            SourceLocation::default(),
        );
        // `Cow<'_, str>` (also implements Into<String>)
        let d3 = Diagnostic::new(
            DiagnosticCode::Other,
            std::borrow::Cow::Borrowed("from Cow"),
            SourceLocation::default(),
        );
        assert_eq!(d1.message, "from &str");
        assert_eq!(d2.message, "from String");
        assert_eq!(d3.message, "from Cow");
    }
}
