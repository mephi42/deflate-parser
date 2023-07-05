extern crate deflate_parser;

#[cfg(test)]
mod test {
    use std::fs::File;
    use std::io::{Read, Seek};
    use std::path::PathBuf;
    use std::{io, str};

    use deflate_parser::data::{CompressedStream, ZlibStream};
    use deflate_parser::error::Error;
    use deflate_parser::{parse, write_data, Settings, Window};

    fn path(name: &str) -> PathBuf {
        let mut result = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        result.push("tests");
        result.push(name);
        result
    }

    fn test_common(name: &str, stream: &Option<CompressedStream>) -> Result<(), Error> {
        let mut expected_json = Vec::new();
        File::open(path(&(name.to_owned() + ".json")))?.read_to_end(&mut expected_json)?;
        let mut actual_json = Vec::new();
        serde_json::to_writer_pretty(&mut actual_json, &stream).expect("to_writer_pretty");
        assert_eq!(
            str::from_utf8(&expected_json).expect("from_utf8)"),
            str::from_utf8(&actual_json).expect("from_utf8")
        );

        let mut expected_data = Vec::new();
        File::open(path(name))?.read_to_end(&mut expected_data)?;
        let mut actual_data = Vec::new();
        {
            let mut extract: File = tempfile::tempfile()?;
            write_data(&mut extract, stream)?;
            extract.seek(io::SeekFrom::Start(0))?;
            extract.read_to_end(&mut actual_data)?;
        }
        assert_eq!(expected_data, actual_data);

        Ok(())
    }

    fn test_gz(name: &str) -> Result<(), Error> {
        let mut stream: Option<CompressedStream> = None;
        let mut window = Window::default();
        parse(
            &mut stream,
            &path(&(name.to_owned() + ".gz")),
            &mut window,
            &Settings {
                bit_offset: 0,
                data: true,
            },
        )?;

        test_common(name, &stream)?;
        Ok(())
    }

    fn test_zlib(name: &str) -> Result<(), Error> {
        let mut stream: Option<CompressedStream> =
            Some(CompressedStream::Zlib(ZlibStream::default()));
        let mut window = Window::default();
        window.append_dictionary_from_file(&mut File::open(path(&(name.to_owned() + ".dict")))?)?;
        parse(
            &mut stream,
            &path(&(name.to_owned() + ".zlib")),
            &mut window,
            &Settings {
                bit_offset: 0,
                data: true,
            },
        )?;

        test_common(name, &stream)?;
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

    #[test]
    fn bbb() -> Result<(), Error> {
        test_zlib("bbb")
    }
}
