use std::fmt;

/// Position in source text for error diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}, column {}", self.line, self.column)
    }
}

/// Error returned when parsing fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub position: Position,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error at {}: {}", self.position, self.message)
    }
}

impl std::error::Error for ParseError {}

impl ParseError {
    pub fn new(message: impl Into<String>, line: usize, column: usize) -> Self {
        ParseError {
            message: message.into(),
            position: Position { line, column },
        }
    }
}
