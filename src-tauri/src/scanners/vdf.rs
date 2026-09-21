//! Minimal parser for Valve's text KeyValues format (libraryfolders.vdf, appmanifest_*.acf).

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Obj(HashMap<String, Value>),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Obj(map) => map.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v),
            Value::Str(_) => None,
        }
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Value::Str(s) => Some(s),
            Value::Obj(_) => None,
        }
    }

    pub fn entries(&self) -> impl Iterator<Item = (&String, &Value)> {
        let map = match self {
            Value::Obj(map) => Some(map),
            Value::Str(_) => None,
        };
        map.into_iter().flatten()
    }
}

/// Parses a whole document into a root object.
pub fn parse(text: &str) -> Value {
    let tokens = tokenize(text);
    let mut pos = 0;
    Value::Obj(parse_object(&tokens, &mut pos))
}

#[derive(Debug, PartialEq)]
enum Token {
    Str(String),
    Open,
    Close,
}

fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => tokens.push(Token::Open),
            '}' => tokens.push(Token::Close),
            '"' => {
                let mut s = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(other) => s.push(other),
                            None => break,
                        },
                        _ => s.push(c),
                    }
                }
                tokens.push(Token::Str(s));
            }
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    tokens
}

fn parse_object(tokens: &[Token], pos: &mut usize) -> HashMap<String, Value> {
    let mut map = HashMap::new();
    while *pos < tokens.len() {
        let key = match &tokens[*pos] {
            Token::Str(s) => s.clone(),
            Token::Close => {
                *pos += 1;
                break;
            }
            Token::Open => {
                *pos += 1;
                continue;
            }
        };
        *pos += 1;
        match tokens.get(*pos) {
            Some(Token::Str(v)) => {
                map.insert(key, Value::Str(v.clone()));
                *pos += 1;
            }
            Some(Token::Open) => {
                *pos += 1;
                map.insert(key, Value::Obj(parse_object(tokens, pos)));
            }
            _ => break,
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_library_folders() {
        let doc = parse(
            r#""libraryfolders"
            {
                "0"
                {
                    "path"		"C:\\Program Files (x86)\\Steam"
                    "apps" { "228980" "123" }
                }
                "1" { "path" "H:\\SteamLibrary" }
            }"#,
        );
        let folders = doc.get("libraryfolders").unwrap();
        let mut paths: Vec<&str> = folders.entries().filter_map(|(_, v)| v.str("path")).collect();
        paths.sort();
        assert_eq!(paths, [r"C:\Program Files (x86)\Steam", r"H:\SteamLibrary"]);
    }
}
