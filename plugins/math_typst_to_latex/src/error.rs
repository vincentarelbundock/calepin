use std::fmt;

/// Errors that can occur during Typst math to LaTeX conversion.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// A parse error with position information.
    Parse { message: String, position: usize },
    /// An unsupported construct was encountered.
    Unsupported(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Parse { message, position } => {
                write!(f, "parse error at position {}: {}", position, message)
            }
            Error::Unsupported(msg) => write!(f, "unsupported: {}", msg),
        }
    }
}

impl std::error::Error for Error {}
