#![allow(dead_code)]
use tablec_core::core::diagnostic::{Diagnostic, DiagnosticCode};

pub fn expect_diagnostic<'a>(errs: &'a [Diagnostic], code: DiagnosticCode) -> &'a Diagnostic {
    errs.iter().find(|d| d.code == code)
        .unwrap_or_else(|| panic!("expected diagnostic with code {:?}, got: {:?}", code, errs))
}
