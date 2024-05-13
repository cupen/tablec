use serde::{Serialize, Deserialize};
use std::str::FromStr;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Constraint {
    pub func: String,
    pub args: Vec<String>,
}

impl FromStr for Constraint {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.starts_with('@') {
            return Err(());
        }

        let s = &s[1..]; // Remove @

        let func: String;
        let args: Vec<String>;

        if let Some(open_paren_idx) = s.find('(') {
            // Case: @func(...)
            func = s[..open_paren_idx].trim().to_string();
            if func.contains(' ') { // Function name itself should not contain spaces before '('
                return Err(());
            }

            let arg_part = &s[open_paren_idx + 1..];
            if !arg_part.ends_with(')') {
                return Err(()); // Missing closing parenthesis
            }
            let arg_str = &arg_part[..arg_part.len() - 1]; // Remove )
            args = arg_str.split(',').map(|s| s.trim().to_string()).collect();
        } else {
            // Case: @func (no parentheses)
            func = s.trim().to_string();
            if func.contains(' ') { // If no parentheses, func must be a single word
                return Err(());
            }
            args = Vec::new();
        }

        if func.is_empty() {
            return Err(());
        }

        Ok(Constraint { func, args })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_ok() {
        let valid_str = "@func(arg1, arg2)";
        let constraint = Constraint::from_str(valid_str).unwrap();
        assert_eq!(constraint.func, "func");
        assert_eq!(constraint.args, vec!["arg1", "arg2"]);
    }

    #[test]
    fn test_fail() {
        let bad_cases = vec![
            "@func arg1, arg2", // Missing parentheses
            "@func(arg1, arg2", // Missing closing parenthesis
            "func(arg1, arg2)", // Missing @ prefix
            "@",                // Just @ with no function name
        ];

        for _case in bad_cases {
            let rs = Constraint::from_str(_case);
            assert!(rs.is_err(), "Expected error for case: '{_case}'");
        }
    }
}