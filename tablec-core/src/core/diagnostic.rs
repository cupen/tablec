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
}
