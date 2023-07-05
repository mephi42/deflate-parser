extern crate clap;
extern crate deflate_parser;
extern crate serde_json;

use std::path::Path;

use clap::{Arg, Command};

use deflate_parser::data::{CompressedStream, DeflateStream, ZlibStream};
use deflate_parser::{parse, Settings};
use std::io::BufWriter;

fn main() {
    let matches = Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(Arg::new("OUTPUT").long("output").takes_value(true))
        .arg(Arg::new("BIT_OFFSET").long("bit-offset").takes_value(true))
        .arg(Arg::new("RAW").long("raw"))
        .arg(Arg::new("RAW_DHT").long("raw-dht").conflicts_with("RAW"))
        .arg(
            Arg::new("ZLIB")
                .long("zlib")
                .conflicts_with_all(&["RAW", "RAW_DHT"]),
        )
        .arg(Arg::new("DATA").long("data"))
        .arg(Arg::new("FILE").required(true).index(1))
        .get_matches();
    let settings = Settings {
        bit_offset: match matches.value_of("BIT_OFFSET") {
            Some(bit_offset_str) => match bit_offset_str.parse::<usize>() {
                Ok(x) => x,
                Err(err) => {
                    eprintln!("{}", err);
                    ::std::process::exit(1);
                }
            },
            None => 0,
        },
        data: matches.is_present("DATA"),
    };
    let output: Box<dyn std::io::Write> = match matches.value_of("OUTPUT") {
        Some(output_path) => match std::fs::File::create(output_path) {
            Ok(x) => Box::new(x),
            Err(err) => {
                eprintln!("{}", err);
                ::std::process::exit(1);
            }
        },
        None => Box::new(std::io::stdout()),
    };
    let raw = matches.is_present("RAW");
    let raw_dht = matches.is_present("RAW_DHT");
    let zlib = matches.is_present("ZLIB");
    let file = matches.value_of("FILE").unwrap();
    let mut stream: Option<CompressedStream> = if raw {
        Some(CompressedStream::Raw(DeflateStream::default()))
    } else if raw_dht {
        Some(CompressedStream::Dht(Box::default()))
    } else if zlib {
        Some(CompressedStream::Zlib(ZlibStream::default()))
    } else {
        None
    };
    let result = parse(&mut stream, Path::new(file), &settings);
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
