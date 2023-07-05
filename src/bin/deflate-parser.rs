extern crate clap;
extern crate deflate_parser;
extern crate serde_json;

use std::fs::File;
use std::path::Path;

use clap::Parser;

use deflate_parser::data::{CompressedStream, DeflateStream, ZlibStream};
use deflate_parser::error::Error;
use deflate_parser::Window;
use deflate_parser::{parse, write_data, Settings};
use std::io::BufWriter;

#[derive(Parser)]
#[command(name = env!("CARGO_PKG_NAME"), author = env!("CARGO_PKG_AUTHORS"), version = env!("CARGO_PKG_DESCRIPTION"))]
struct Args {
    #[arg(long)]
    output: Option<String>,

    #[arg(long)]
    extract: Option<String>,

    #[arg(long)]
    dictionary: Option<String>,

    #[arg(long, default_value_t = 0)]
    bit_offset: usize,

    #[arg(long)]
    raw: bool,

    #[arg(long)]
    raw_dht: bool,

    #[arg(long)]
    zlib: bool,

    #[arg(long)]
    data: bool,

    file: String,
}

fn main() -> Result<(), Error> {
    let args = Args::parse();
    let settings = Settings {
        bit_offset: args.bit_offset,
        data: args.data || args.extract.is_some(),
    };
    let output: Box<dyn std::io::Write> = match args.output {
        Some(output_path) => Box::new(std::fs::File::create(output_path)?),
        None => Box::new(std::io::stdout()),
    };
    let mut stream: Option<CompressedStream> = if args.raw {
        Some(CompressedStream::Raw(DeflateStream::default()))
    } else if args.raw_dht {
        Some(CompressedStream::Dht(Box::default()))
    } else if args.zlib {
        Some(CompressedStream::Zlib(ZlibStream::default()))
    } else {
        None
    };
    let mut window = Window::default();
    if let Some(dictionary) = args.dictionary {
        window.append_dictionary_from_file(&mut File::open(dictionary)?)?;
    }
    let result = parse(&mut stream, Path::new(&args.file), &mut window, &settings);
    serde_json::to_writer_pretty(BufWriter::new(output), &stream)?;
    match result {
        Ok(()) => {}
        Err(err) => {
            serde_json::to_string_pretty(&err)?;
        }
    }
    if let Some(extract) = &args.extract {
        let mut f = File::create(extract)?;
        write_data(&mut f, &stream)?;
    }
    Ok(())
}
