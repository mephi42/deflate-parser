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
        .arg(Arg::with_name("output")
            .long("output")
            .takes_value(true))
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
            Ok(x) => x,
            Err(err) => {
                eprintln!("{}", err);
                ::std::process::exit(1);
            }
        }
        None => 0
    };
    let mut output: Box<std::io::Write> = match matches.value_of("output") {
        Some(output_path) => match std::fs::File::create(output_path) {
            Ok(x) => Box::new(x),
            Err(err) => {
                eprintln!("{}", err);
                ::std::process::exit(1);
            }
        },
        None => Box::new(std::io::stdout())
    };
    let raw = matches.is_present("raw");
    let file = matches.value_of("FILE").unwrap();
    let mut stream: Option<CompressedStream> = None;
    let result = parse(&mut stream, Path::new(file), bit_offset, raw);
    let out_json = serde_json::to_string_pretty(&stream).expect("to_string_pretty");
    match write!(output, "{}", out_json) {
        Ok(_) => {}
        Err(err) => {
            eprintln!("{}", err);
            ::std::process::exit(1);
        }
    };
    match result {
        Ok(()) => {}
        Err(err) => {
            let err_json = serde_json::to_string_pretty(&err).expect("to_string_pretty");
            eprintln!("{}", err_json);
            ::std::process::exit(1);
        }
    }
}
