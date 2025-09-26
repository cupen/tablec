pub enum TokenType {
    Symbol,
    Word,
}

pub struct Token<'a> {
    pub value: &'a str,
    pub token_type: TokenType,
    pub start: usize,
    pub end: usize,
}

pub fn scan_tokens(s: &str) -> Vec<Token<'_>> {
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
                panic!("Unexpected character: '{}' in \"{}\"", c, s);
            }
        }
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parse_tokens() {
        let tokens = scan_tokens("array<int>, map<string, int>, array<float>");

        let actual: Vec<&str> = tokens.iter().map(|t| t.value).collect();

        let expected = vec![
            "array", "<", "int", ">", ",",
            "map", "<", "string", ",",
            "int", ">", ",",
            "array", "<", "float", ">",
        ];
        assert_eq!(expected, actual);
    }
}
