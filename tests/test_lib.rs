extern crate deflate_parser;

#[cfg(test)]
mod test {
    use std::fs::File;
    use std::io::{Read, Seek};
    use std::path::PathBuf;
    use std::{io, str};

    use deflate_parser::data::CompressedStream;
    use deflate_parser::error::Error;
    use deflate_parser::{parse, write_data, Settings};

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

        let mut expected_json = Vec::new();
        File::open(path(&(name.to_owned() + ".json")))?.read_to_end(&mut expected_json)?;
        let mut actual_json = Vec::new();
        serde_json::to_writer_pretty(&mut actual_json, &result).expect("to_writer_pretty");
        assert_eq!(
            str::from_utf8(&expected_json).expect("from_utf8)"),
            str::from_utf8(&actual_json).expect("from_utf8")
        );

        let mut expected_data = Vec::new();
        File::open(path(name))?.read_to_end(&mut expected_data)?;
        let mut actual_data = Vec::new();
        {
            let mut extract: File = tempfile::tempfile()?;
            write_data(&mut extract, &result)?;
            extract.seek(io::SeekFrom::Start(0))?;
            extract.read_to_end(&mut actual_data)?;
        }
        assert_eq!(expected_data, actual_data);
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

    #[test]
    fn stored() -> Result<(), Error> {
        test_gz("stored")
    }
}
