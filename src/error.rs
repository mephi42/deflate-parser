#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Error {
    Io(String),
    Parse(ParseError),
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

impl From<ParseError> for Error {
    fn from(error: ParseError) -> Self {
        Error::Parse(error)
    }
}
