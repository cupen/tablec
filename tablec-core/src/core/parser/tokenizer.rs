use crate::core::diagnostic::{Diagnostic, DiagnosticCode, SourceLocation};

#[derive(Debug)]
pub enum TokenType {
    Symbol,
    Word,
}

#[derive(Debug)]
pub struct Token<'a> {
    pub value: &'a str,
    pub token_type: TokenType,
    pub start: usize,
    pub end: usize,
}

pub fn scan_tokens<'a>(s: &'a str, loc: SourceLocation) -> Result<Vec<Token<'a>>, Diagnostic> {
    let mut tokens = Vec::new();
    let mut chars = s.char_indices().peekable();

    while let Some((i, c)) = chars.next() {
        match c {
            '<' | '>' | ',' | '{' | '}' | '[' | ']' | ':' => {
                tokens.push(Token {
                    value: &s[i..i + c.len_utf8()],
                    token_type: TokenType::Symbol,
                    start: i,
                    end: i + c.len_utf8(),
                });
            }

            _ if c.is_whitespace() => {
                // skip
            }

            _ if c.is_alphanumeric() => {
                let start = i;
                let mut end = i + c.len_utf8();
                while let Some((j, c_next)) = chars.peek() {
                    if c_next.is_alphanumeric() {
                        end = j + c_next.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token {
                    value: &s[start..end],
                    token_type: TokenType::Word,
                    start,
                    end,
                });
            }

            _ => {
                return Err(Diagnostic::new(
                    DiagnosticCode::TokenizerUnexpectedChar,
                    format!("Unexpected character: '{}'", c),
                    SourceLocation {
                        line: loc.line,
                        column: loc.column.map(|x| x + i as u32),
                        ..Default::default()
                    },
                ));
            }
        }
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::diagnostic::*;
    #[test]
    fn test_parse_tokens() {
        let tokens = scan_tokens(
            "array<int>, map<string, int>, array<float>",
            SourceLocation::default(),
        )
        .unwrap();

        let actual: Vec<&str> = tokens.iter().map(|t| t.value).collect();

        let expected = vec![
            "array", "<", "int", ">", ",", "map", "<", "string", ",", "int", ">", ",", "array",
            "<", "float", ">",
        ];
        assert_eq!(expected, actual);
    }
}

#[cfg(test)]
mod result_tests {
    use super::*;
    use crate::core::diagnostic::*;

    #[test]
    fn empty_string_returns_empty_vec() {
        let loc = SourceLocation::default();
        let tokens = scan_tokens("", loc).unwrap();
        assert!(tokens.is_empty());
    }

    #[test]
    fn symbols_only_tokens() {
        let loc = SourceLocation::default();
        let tokens = scan_tokens("[]<>{},:", loc).unwrap();
        assert_eq!(
            tokens.iter().map(|t| t.value).collect::<Vec<_>>(),
            vec!["[", "]", "<", ">", "{", "}", ",", ":"]
        );
    }

    #[test]
    fn unrecognized_char_returns_diagnostic() {
        let loc = SourceLocation {
            line: Some(1),
            column: Some(4),
            ..Default::default()
        };
        let err = scan_tokens("int🙂", loc).unwrap_err();
        assert_eq!(err.code, DiagnosticCode::TokenizerUnexpectedChar);
        assert_eq!(err.location.line, Some(1));
        assert_eq!(err.location.column, Some(7));
        assert!(err.message.contains("🙂"));
    }

    #[test]
    fn existing_happy_path_preserved() {
        let loc = SourceLocation::default();
        let tokens = scan_tokens("array<int>, map<string, int>, array<float>", loc).unwrap();
        assert!(tokens.len() > 5);
    }
}
