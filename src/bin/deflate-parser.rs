extern crate clap;
extern crate deflate_parser;
extern crate serde_json;

use std::path::Path;

use clap::Parser;

use deflate_parser::data::{CompressedStream, DeflateStream, ZlibStream};
use deflate_parser::{parse, Settings};
use std::io::BufWriter;

#[derive(Parser)]
#[command(name = env!("CARGO_PKG_NAME"), author = env!("CARGO_PKG_AUTHORS"), version = env!("CARGO_PKG_DESCRIPTION"))]
struct Args {
    #[arg(long)]
    output: Option<String>,

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

fn main() {
    let args = Args::parse();
    let settings = Settings {
        bit_offset: args.bit_offset,
        data: args.data,
    };
    let output: Box<dyn std::io::Write> = match args.output {
        Some(output_path) => match std::fs::File::create(output_path) {
            Ok(x) => Box::new(x),
            Err(err) => {
                eprintln!("{}", err);
                ::std::process::exit(1);
            }
        },
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
    let result = parse(&mut stream, Path::new(&args.file), &settings);
    match serde_json::to_writer_pretty(BufWriter::new(output), &stream) {
        Ok(_) => {}
        Err(err) => {
            eprintln!("{}", err);
            ::std::process::exit(1);
        }
    }
    match result {
        Ok(()) => {}
        Err(err) => {
            let err_json = serde_json::to_string_pretty(&err).expect("to_string_pretty");
            eprintln!("{}", err_json);
            ::std::process::exit(1);
        }
    }
}
