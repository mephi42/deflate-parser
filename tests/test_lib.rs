extern crate deflate_parser;
extern crate tempfile;

#[cfg(test)]
mod test {
    use std::io::Write;

    use tempfile::tempfile;

    use deflate_parser::data::{CompressedStream, GzipStream};
    use deflate_parser::parse_file;
    use deflate_parser::error::Error;

    #[test]
    fn hello() -> Result<(), Error> {
        let gz_data = [
            0x1f, 0x8b, 0x08, 0x00, 0xd1, 0x9f, 0x38, 0x5c,
            0x02, 0x03, 0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0xe7,
            0x02, 0x00, 0x20, 0x30, 0x3a, 0x36, 0x06, 0x00,
            0x00, 0x00];
        let mut gz_file = tempfile()?;
        gz_file.write(&gz_data)?;
        let mut result: Option<CompressedStream> = None;
        parse_file(&mut result, gz_file, 0)?;
        let stream = result.expect("CompressedStream is None");
        let gzip = match stream {
            CompressedStream::Gzip(x) => x,
            _ => panic!("CompressedStream is not a Gzip"),
        };
        let _deflate = gzip.deflate.expect("DeflateStream is None");
        Ok(())
    }
}
