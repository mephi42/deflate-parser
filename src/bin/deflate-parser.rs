extern crate clap;
extern crate deflate_parser;
extern crate serde_json;

use std::path::Path;

use clap::{App, Arg};

use deflate_parser::data::CompressedStream;
use deflate_parser::parse;

fn main() {
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(Arg::with_name("bit-offset")
            .long("bit-offset")
            .takes_value(true))
        .arg(Arg::with_name("raw")
            .long("raw"))
        .arg(Arg::with_name("FILE")
            .required(true)
            .index(1))
        .get_matches();
    let bit_offset = match matches.value_of("bit-offset") {
        Some(bit_offset_str) => match bit_offset_str.parse::<usize>() {
            Ok(bit_offset) => bit_offset,
            Err(err) => {
                eprintln!("{}", err);
                ::std::process::exit(1);
            }
        }
        None => 0
    };
    let raw = matches.is_present("raw");
    let file = matches.value_of("FILE").unwrap();
    let mut stream: Option<CompressedStream> = None;
    let result = parse(&mut stream, Path::new(file), bit_offset, raw);
    println!("{}", serde_json::to_string_pretty(&stream).expect("to_string_pretty"));
    match result {
        Ok(()) => {}
        Err(err) => {
            eprintln!(
                "{}", serde_json::to_string_pretty(&err).expect("to_string_pretty"));
            ::std::process::exit(1);
        }
    }
}
