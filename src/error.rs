use data::{HuffmanCode, HuffmanTree};

#[derive(Serialize)]
pub enum Error {
    Io(String),
    Parse(ParseError),
    HuffmanCodeLengths(HuffmanTreeError<u8>),
}

#[derive(Serialize)]
pub struct ParseError {
    pub pos: usize,
    pub msg: String,
}

#[derive(Serialize)]
pub struct HuffmanTreeError<T> {
    pub tree: HuffmanTree<T>,
    pub codes: Vec<HuffmanCode<T>>,
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
