//! # toml-rs
//!
//! A zero-dependency TOML v1.0.0 parser for Rust.
//!
//! ## Usage
//!
//! ```rust
//! use toml_rs::{parse, TomlValue};
//!
//! let input = r#"
//! [agent]
//! name = "my-agent"
//! version = 1
//! enabled = true
//! "#;
//!
//! let value = parse(input).unwrap();
//! if let TomlValue::Table(table) = &value {
//!     if let Some(TomlValue::Table(agent)) = table.get("agent") {
//!         assert_eq!(agent.get("name"), Some(&TomlValue::String("my-agent".into())));
//!     }
//! }
//! ```

pub mod error;

pub use error::{ParseError, Position};

use std::collections::HashMap;

// ─── Value types ─────────────────────────────────────────────────────────────

/// A TOML value.
#[derive(Debug, Clone, PartialEq)]
pub enum TomlValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    /// RFC 3339 / ISO 8601 datetime stored as the raw string.
    DateTime(String),
    Array(Vec<TomlValue>),
    Table(HashMap<String, TomlValue>),
}

// ─── Lexer ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Token {
    // Structural
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,
    Equals,
    Comma,
    Dot,
    Newline,
    Eof,
    // Values
    Key(String),
    StringLit(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    DateTime(String),
}

struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Lexer { src: src.as_bytes(), pos: 0, line: 1, col: 1 }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn peek2(&self) -> Option<u8> {
        self.src.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> Option<u8> {
        let b = self.src.get(self.pos).copied()?;
        self.pos += 1;
        if b == b'\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(b)
    }

    fn err(&self, msg: impl Into<String>) -> ParseError {
        ParseError::new(msg, self.line, self.col)
    }

    /// Skip a `#` comment to end of line.
    fn skip_comment(&mut self) {
        while let Some(b) = self.peek() {
            if b == b'\n' { break; }
            self.advance();
        }
    }

    /// Skip whitespace (space, tab, CR) but not newlines.
    fn skip_inline_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ') | Some(b'\t') | Some(b'\r')) {
            self.advance();
        }
    }

    /// Read a basic double-quoted string (handles escape sequences).
    fn read_basic_string(&mut self) -> Result<String, ParseError> {
        // Opening `"` already consumed by caller.
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return Err(self.err("unterminated string")),
                Some(b'"') => { self.advance(); break; }
                Some(b'\\') => {
                    self.advance();
                    match self.advance() {
                        Some(b'b')  => out.push('\x08'),
                        Some(b't')  => out.push('\t'),
                        Some(b'n')  => out.push('\n'),
                        Some(b'f')  => out.push('\x0c'),
                        Some(b'r')  => out.push('\r'),
                        Some(b'"')  => out.push('"'),
                        Some(b'\\') => out.push('\\'),
                        Some(b'u')  => out.push(self.read_unicode_escape(4)?),
                        Some(b'U')  => out.push(self.read_unicode_escape(8)?),
                        Some(b'\n') | Some(b' ') | Some(b'\t') => {
                            // Line-ending backslash: skip whitespace/newlines.
                            // Back up one if it was newline (already advanced).
                            self.skip_multiline_ws();
                        }
                        Some(c) => return Err(self.err(format!("invalid escape \\{}", c as char))),
                        None => return Err(self.err("unterminated escape")),
                    }
                }
                Some(b'\n') | Some(b'\r') => {
                    return Err(self.err("newline in basic string"));
                }
                _ => {
                    let b = self.advance().unwrap();
                    // Multi-byte UTF-8 handling: just push bytes directly.
                    out.push(b as char);
                    // For multi-byte sequences, read continuation bytes.
                    if b >= 0x80 {
                        // Already pushed first byte as char; revert and do proper decode.
                        out.pop();
                        let start = self.pos - 1;
                        let len = utf8_char_len(b);
                        for _ in 1..len {
                            self.advance();
                        }
                        let slice = &self.src[start..start + len];
                        let s = std::str::from_utf8(slice)
                            .map_err(|_| self.err("invalid UTF-8"))?;
                        out.push_str(s);
                    }
                }
            }
        }
        Ok(out)
    }

    fn skip_multiline_ws(&mut self) {
        while matches!(self.peek(), Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r')) {
            self.advance();
        }
    }

    fn read_unicode_escape(&mut self, digits: usize) -> Result<char, ParseError> {
        let mut val = 0u32;
        for _ in 0..digits {
            let b = self.advance().ok_or_else(|| self.err("unterminated unicode escape"))?;
            let d = hex_digit(b).ok_or_else(|| self.err("invalid hex in unicode escape"))?;
            val = val * 16 + d;
        }
        char::from_u32(val).ok_or_else(|| self.err("invalid unicode codepoint"))
    }

    /// Read a multi-line basic string (after `"""` already consumed).
    fn read_ml_basic_string(&mut self) -> Result<String, ParseError> {
        // Trim first newline if present.
        if self.peek() == Some(b'\n') { self.advance(); }
        else if self.peek() == Some(b'\r') && self.peek2() == Some(b'\n') {
            self.advance(); self.advance();
        }
        let mut out = String::new();
        loop {
            if self.peek() == Some(b'"') && self.src.get(self.pos+1) == Some(&b'"') && self.src.get(self.pos+2) == Some(&b'"') {
                self.advance(); self.advance(); self.advance();
                break;
            }
            match self.peek() {
                None => return Err(self.err("unterminated multi-line string")),
                Some(b'\\') => {
                    self.advance();
                    match self.peek() {
                        Some(b'\n') | Some(b' ') | Some(b'\t') => {
                            self.advance();
                            self.skip_multiline_ws();
                        }
                        _ => {
                            match self.advance() {
                                Some(b'b')  => out.push('\x08'),
                                Some(b't')  => out.push('\t'),
                                Some(b'n')  => out.push('\n'),
                                Some(b'f')  => out.push('\x0c'),
                                Some(b'r')  => out.push('\r'),
                                Some(b'"')  => out.push('"'),
                                Some(b'\\') => out.push('\\'),
                                Some(b'u')  => out.push(self.read_unicode_escape(4)?),
                                Some(b'U')  => out.push(self.read_unicode_escape(8)?),
                                Some(c) => return Err(self.err(format!("invalid escape \\{}", c as char))),
                                None => return Err(self.err("unterminated escape")),
                            }
                        }
                    }
                }
                _ => {
                    let b = self.advance().unwrap();
                    if b >= 0x80 {
                        let start = self.pos - 1;
                        let len = utf8_char_len(b);
                        for _ in 1..len { self.advance(); }
                        let slice = &self.src[start..start + len];
                        let s = std::str::from_utf8(slice)
                            .map_err(|_| self.err("invalid UTF-8"))?;
                        out.push_str(s);
                    } else {
                        out.push(b as char);
                    }
                }
            }
        }
        Ok(out)
    }

    /// Read a literal single-quoted string (no escapes).
    fn read_literal_string(&mut self) -> Result<String, ParseError> {
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return Err(self.err("unterminated literal string")),
                Some(b'\'') => { self.advance(); break; }
                Some(b'\n') | Some(b'\r') => return Err(self.err("newline in literal string")),
                _ => {
                    let b = self.advance().unwrap();
                    if b >= 0x80 {
                        let start = self.pos - 1;
                        let len = utf8_char_len(b);
                        for _ in 1..len { self.advance(); }
                        let slice = &self.src[start..start + len];
                        let s = std::str::from_utf8(slice)
                            .map_err(|_| self.err("invalid UTF-8"))?;
                        out.push_str(s);
                    } else {
                        out.push(b as char);
                    }
                }
            }
        }
        Ok(out)
    }

    /// Read a multi-line literal string (after `'''` consumed).
    fn read_ml_literal_string(&mut self) -> Result<String, ParseError> {
        if self.peek() == Some(b'\n') { self.advance(); }
        else if self.peek() == Some(b'\r') && self.peek2() == Some(b'\n') {
            self.advance(); self.advance();
        }
        let mut out = String::new();
        loop {
            if self.peek() == Some(b'\'') && self.src.get(self.pos+1) == Some(&b'\'') && self.src.get(self.pos+2) == Some(&b'\'') {
                self.advance(); self.advance(); self.advance();
                break;
            }
            match self.peek() {
                None => return Err(self.err("unterminated multi-line literal string")),
                _ => {
                    let b = self.advance().unwrap();
                    if b >= 0x80 {
                        let start = self.pos - 1;
                        let len = utf8_char_len(b);
                        for _ in 1..len { self.advance(); }
                        let slice = &self.src[start..start + len];
                        let s = std::str::from_utf8(slice)
                            .map_err(|_| self.err("invalid UTF-8"))?;
                        out.push_str(s);
                    } else {
                        out.push(b as char);
                    }
                }
            }
        }
        Ok(out)
    }

    /// Read a bare key (alphanumeric + `-` + `_`).
    fn read_bare_key(&mut self, first: u8) -> String {
        let mut key = String::new();
        key.push(first as char);
        while let Some(b) = self.peek() {
            if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' {
                self.advance();
                key.push(b as char);
            } else {
                break;
            }
        }
        key
    }

    /// Read a number or datetime starting with a digit (or sign).
    fn read_number_or_datetime(&mut self, first: u8) -> Result<Token, ParseError> {
        let mut raw = String::new();
        raw.push(first as char);

        // Collect all characters that could be part of a number or datetime.
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-' | b'_' | b'T' | b'Z' | b':') {
                self.advance();
                if b != b'_' { raw.push(b as char); }
                else { /* underscore separator: skip in raw */ }
            } else {
                break;
            }
        }

        // Check for datetime: must contain '-' after first 4 digits and a 'T' or ':'
        // Simple heuristic: if it has a 'T' or matches YYYY-MM-DD pattern.
        let is_datetime = raw.len() >= 10
            && raw.as_bytes().get(4) == Some(&b'-')
            && raw.as_bytes().get(7) == Some(&b'-');

        if is_datetime {
            return Ok(Token::DateTime(raw));
        }

        // Try integer first (no dot or e).
        if !raw.contains('.') && !raw.to_lowercase().contains('e') {
            match raw.parse::<i64>() {
                Ok(n) => return Ok(Token::Integer(n)),
                Err(_) => {}
            }
        }

        // Try float.
        match raw.parse::<f64>() {
            Ok(f) => Ok(Token::Float(f)),
            Err(_) => Err(ParseError::new(
                format!("invalid number: {}", raw),
                self.line,
                self.col,
            )),
        }
    }

    /// Return the next token and position (line, col before consuming).
    fn next_token(&mut self) -> Result<(Token, usize, usize), ParseError> {
        loop {
            self.skip_inline_whitespace();
            let line = self.line;
            let col = self.col;

            let b = match self.peek() {
                None => return Ok((Token::Eof, line, col)),
                Some(b) => b,
            };

            match b {
                b'\n' => {
                    self.advance();
                    return Ok((Token::Newline, line, col));
                }
                b'\r' => {
                    self.advance();
                    if self.peek() == Some(b'\n') { self.advance(); }
                    return Ok((Token::Newline, line, col));
                }
                b'#' => {
                    self.advance();
                    self.skip_comment();
                    continue;
                }
                b'[' => { self.advance(); return Ok((Token::LeftBracket, line, col)); }
                b']' => { self.advance(); return Ok((Token::RightBracket, line, col)); }
                b'{' => { self.advance(); return Ok((Token::LeftBrace, line, col)); }
                b'}' => { self.advance(); return Ok((Token::RightBrace, line, col)); }
                b'=' => { self.advance(); return Ok((Token::Equals, line, col)); }
                b',' => { self.advance(); return Ok((Token::Comma, line, col)); }
                b'.' => { self.advance(); return Ok((Token::Dot, line, col)); }
                b'"' => {
                    self.advance();
                    // Check for triple quote.
                    if self.peek() == Some(b'"') && self.src.get(self.pos+1) == Some(&b'"') {
                        self.advance(); self.advance();
                        let s = self.read_ml_basic_string()?;
                        return Ok((Token::StringLit(s), line, col));
                    }
                    let s = self.read_basic_string()?;
                    return Ok((Token::StringLit(s), line, col));
                }
                b'\'' => {
                    self.advance();
                    if self.peek() == Some(b'\'') && self.src.get(self.pos+1) == Some(&b'\'') {
                        self.advance(); self.advance();
                        let s = self.read_ml_literal_string()?;
                        return Ok((Token::StringLit(s), line, col));
                    }
                    let s = self.read_literal_string()?;
                    return Ok((Token::StringLit(s), line, col));
                }
                b'0'..=b'9' | b'-' | b'+' => {
                    self.advance();
                    let tok = self.read_number_or_datetime(b)?;
                    return Ok((tok, line, col));
                }
                _ if b.is_ascii_alphanumeric() || b == b'_' => {
                    self.advance();
                    let key = self.read_bare_key(b);
                    // Resolve keywords.
                    let tok = match key.as_str() {
                        "true"  => Token::Boolean(true),
                        "false" => Token::Boolean(false),
                        _       => Token::Key(key),
                    };
                    return Ok((tok, line, col));
                }
                _ => {
                    return Err(ParseError::new(
                        format!("unexpected character: {:?}", b as char),
                        line, col,
                    ));
                }
            }
        }
    }

    /// Tokenise everything into a Vec.
    fn tokenise(mut self) -> Result<Vec<(Token, usize, usize)>, ParseError> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            let is_eof = tok.0 == Token::Eof;
            tokens.push(tok);
            if is_eof { break; }
        }
        Ok(tokens)
    }
}

// ─── Parser ──────────────────────────────────────────────────────────────────

struct Parser {
    tokens: Vec<(Token, usize, usize)>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<(Token, usize, usize)>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos].0
    }

    fn peek_pos(&self) -> (usize, usize) {
        (self.tokens[self.pos].1, self.tokens[self.pos].2)
    }

    fn advance(&mut self) -> &Token {
        let t = &self.tokens[self.pos].0;
        if self.pos + 1 < self.tokens.len() { self.pos += 1; }
        t
    }

    fn err(&self, msg: impl Into<String>) -> ParseError {
        let (line, col) = self.peek_pos();
        ParseError::new(msg, line, col)
    }

    /// Skip newlines.
    fn skip_newlines(&mut self) {
        while self.peek() == &Token::Newline { self.advance(); }
    }

    /// Expect and consume a newline or EOF (end of key-value line).
    fn expect_eol(&mut self) -> Result<(), ParseError> {
        match self.peek() {
            Token::Newline | Token::Eof => { self.advance(); Ok(()) }
            Token::RightBrace => Ok(()), // allow inside inline tables
            _ => Err(self.err("expected newline or end of input after value")),
        }
    }

    /// Parse a quoted or bare key segment (returns string).
    fn parse_key_segment(&mut self) -> Result<String, ParseError> {
        match self.peek().clone() {
            Token::Key(k) => { self.advance(); Ok(k) }
            Token::StringLit(s) => { self.advance(); Ok(s) }
            // Integers can be bare keys too (e.g. `1 = "one"` is valid TOML).
            Token::Integer(n) => { self.advance(); Ok(n.to_string()) }
            Token::Boolean(b) => { self.advance(); Ok(b.to_string()) }
            _ => Err(self.err("expected key")),
        }
    }

    /// Parse a dotted key path (returns Vec of segments).
    fn parse_key_path(&mut self) -> Result<Vec<String>, ParseError> {
        let mut path = vec![self.parse_key_segment()?];
        while self.peek() == &Token::Dot {
            self.advance();
            path.push(self.parse_key_segment()?);
        }
        Ok(path)
    }

    /// Parse a TOML value.
    fn parse_value(&mut self) -> Result<TomlValue, ParseError> {
        match self.peek().clone() {
            Token::StringLit(s) => { self.advance(); Ok(TomlValue::String(s)) }
            Token::Integer(n)   => { self.advance(); Ok(TomlValue::Integer(n)) }
            Token::Float(f)     => { self.advance(); Ok(TomlValue::Float(f)) }
            Token::Boolean(b)   => { self.advance(); Ok(TomlValue::Boolean(b)) }
            Token::DateTime(d)  => { self.advance(); Ok(TomlValue::DateTime(d)) }
            Token::LeftBracket  => self.parse_array(),
            Token::LeftBrace    => self.parse_inline_table(),
            _ => Err(self.err("expected value")),
        }
    }

    /// Parse an array `[ value, value, ... ]`.
    fn parse_array(&mut self) -> Result<TomlValue, ParseError> {
        self.advance(); // consume `[`
        let mut items = Vec::new();
        loop {
            self.skip_newlines();
            if self.peek() == &Token::RightBracket {
                self.advance();
                break;
            }
            items.push(self.parse_value()?);
            self.skip_newlines();
            match self.peek() {
                Token::Comma => { self.advance(); }
                Token::RightBracket => { self.advance(); break; }
                _ => return Err(self.err("expected ',' or ']' in array")),
            }
        }
        Ok(TomlValue::Array(items))
    }

    /// Parse an inline table `{ key = value, ... }`.
    fn parse_inline_table(&mut self) -> Result<TomlValue, ParseError> {
        self.advance(); // consume `{`
        let mut map = HashMap::new();
        if self.peek() == &Token::RightBrace {
            self.advance();
            return Ok(TomlValue::Table(map));
        }
        loop {
            let path = self.parse_key_path()?;
            if self.peek() != &Token::Equals {
                return Err(self.err("expected '=' in inline table"));
            }
            self.advance();
            let val = self.parse_value()?;
            set_nested(&mut map, &path, val)
                .map_err(|e| self.err(e))?;
            match self.peek() {
                Token::Comma => { self.advance(); }
                Token::RightBrace => { self.advance(); break; }
                _ => return Err(self.err("expected ',' or '}' in inline table")),
            }
        }
        Ok(TomlValue::Table(map))
    }

    /// Top-level parse: returns the root table.
    fn parse_document(&mut self) -> Result<HashMap<String, TomlValue>, ParseError> {
        let mut root: HashMap<String, TomlValue> = HashMap::new();
        // current_table points into root via a path.
        let mut current_path: Vec<String> = Vec::new();

        self.skip_newlines();

        loop {
            match self.peek().clone() {
                Token::Eof => break,

                Token::Newline => { self.advance(); }

                // Table header `[section]`
                Token::LeftBracket => {
                    self.advance();
                    let path = self.parse_key_path()?;
                    if self.peek() != &Token::RightBracket {
                        return Err(self.err("expected ']'"));
                    }
                    self.advance();
                    // Ensure path exists as table.
                    ensure_table_path(&mut root, &path)
                        .map_err(|e| self.err(e))?;
                    current_path = path;
                    self.skip_newlines();
                }

                // Key = value
                Token::Key(_) | Token::StringLit(_) | Token::Integer(_) | Token::Boolean(_) => {
                    let path = self.parse_key_path()?;
                    if self.peek() != &Token::Equals {
                        return Err(self.err("expected '='"));
                    }
                    self.advance();
                    let val = self.parse_value()?;

                    // Insert into current table.
                    let target = get_table_mut(&mut root, &current_path)
                        .ok_or_else(|| self.err("current table not found"))?;
                    set_nested(target, &path, val)
                        .map_err(|e| self.err(e))?;

                    // Consume newline/EOF.
                    match self.peek() {
                        Token::Newline | Token::Eof => { self.advance(); }
                        _ => return Err(self.err("expected newline after key-value pair")),
                    }
                }

                t => return Err(self.err(format!("unexpected token: {:?}", t))),
            }
        }

        Ok(root)
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Return the number of bytes in a UTF-8 sequence given the first byte.
fn utf8_char_len(b: u8) -> usize {
    if b < 0x80 { 1 }
    else if b < 0xE0 { 2 }
    else if b < 0xF0 { 3 }
    else { 4 }
}

fn hex_digit(b: u8) -> Option<u32> {
    match b {
        b'0'..=b'9' => Some((b - b'0') as u32),
        b'a'..=b'f' => Some((b - b'a' + 10) as u32),
        b'A'..=b'F' => Some((b - b'A' + 10) as u32),
        _ => None,
    }
}

/// Set a value at a dotted path inside a table, creating sub-tables as needed.
fn set_nested(
    table: &mut HashMap<String, TomlValue>,
    path: &[String],
    val: TomlValue,
) -> Result<(), String> {
    if path.is_empty() {
        return Err("empty key path".into());
    }
    if path.len() == 1 {
        if table.contains_key(&path[0]) {
            return Err(format!("duplicate key: {}", path[0]));
        }
        table.insert(path[0].clone(), val);
        return Ok(());
    }
    // Intermediate key: must be (or become) a Table.
    let entry = table.entry(path[0].clone()).or_insert_with(|| {
        TomlValue::Table(HashMap::new())
    });
    if let TomlValue::Table(t) = entry {
        set_nested(t, &path[1..], val)
    } else {
        Err(format!("key '{}' already exists as non-table", path[0]))
    }
}

/// Ensure the given path exists as nested tables (creating them if absent).
fn ensure_table_path(
    root: &mut HashMap<String, TomlValue>,
    path: &[String],
) -> Result<(), String> {
    if path.is_empty() { return Ok(()); }
    let entry = root.entry(path[0].clone()).or_insert_with(|| {
        TomlValue::Table(HashMap::new())
    });
    if let TomlValue::Table(t) = entry {
        ensure_table_path(t, &path[1..])
    } else {
        Err(format!("key '{}' is not a table", path[0]))
    }
}

/// Get a mutable reference to the table at the given path.
fn get_table_mut<'a>(
    root: &'a mut HashMap<String, TomlValue>,
    path: &[String],
) -> Option<&'a mut HashMap<String, TomlValue>> {
    if path.is_empty() { return Some(root); }
    match root.get_mut(&path[0])? {
        TomlValue::Table(t) => get_table_mut(t, &path[1..]),
        _ => None,
    }
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Parse a TOML string into a [`TomlValue::Table`].
///
/// # Errors
/// Returns [`ParseError`] with line/column info on invalid TOML.
pub fn parse(input: &str) -> Result<TomlValue, ParseError> {
    let tokens = Lexer::new(input).tokenise()?;
    let mut parser = Parser::new(tokens);
    let table = parser.parse_document()?;
    Ok(TomlValue::Table(table))
}
