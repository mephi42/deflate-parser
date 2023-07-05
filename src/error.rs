#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Error {
    Io(String),
    Utf8(String),
    Parse(ParseError),
    Serde(String),
}

#[derive(Debug, Serialize)]
pub struct ParseError {
    pub pos: usize,
    pub msg: String,
}

impl From<::std::io::Error> for Error {
    fn from(error: ::std::io::Error) -> Self {
        Error::Io(error.to_string())
    }
}

impl From<::std::str::Utf8Error> for Error {
    fn from(error: ::std::str::Utf8Error) -> Self {
        Error::Utf8(error.to_string())
    }
}

impl From<ParseError> for Error {
    fn from(error: ParseError) -> Self {
        Error::Parse(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error::Serde(error.to_string())
    }
}
