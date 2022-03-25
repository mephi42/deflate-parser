extern crate deflate_parser;

#[cfg(test)]
mod test {
    use std::fs::File;
    use std::io::Read;
    use std::path::PathBuf;
    use std::str;

    use deflate_parser::data::CompressedStream;
    use deflate_parser::error::Error;
    use deflate_parser::{parse, Settings};

    fn path(name: &str) -> PathBuf {
        let mut result = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        result.push("tests");
        result.push(name);
        result
    }

    fn test_gz(name: &str) -> Result<(), Error> {
        let mut result: Option<CompressedStream> = None;
        parse(
            &mut result,
            &path(&(name.to_owned() + ".gz")),
            &Settings {
                bit_offset: 0,
                data: true,
            },
        )?;
        let mut expected = Vec::new();
        File::open(&path(&(name.to_owned() + ".json")))?.read_to_end(&mut expected)?;
        let mut actual = Vec::new();
        serde_json::to_writer_pretty(&mut actual, &result).expect("to_writer_pretty");
        assert_eq!(
            str::from_utf8(&expected).expect("from_utf8)"),
            str::from_utf8(&actual).expect("from_utf8")
        );
        Ok(())
    }

    #[test]
    fn hello() -> Result<(), Error> {
        test_gz("hello")
    }

    #[test]
    fn aaa() -> Result<(), Error> {
        test_gz("aaa")
    }
}
