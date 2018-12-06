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
        .arg(Arg::with_name("FILE")
            .required(true)
            .index(1))
        .get_matches();
    let file = matches.value_of("FILE").unwrap();
    let mut stream: Option<CompressedStream> = None;
    let result = parse(&mut stream, Path::new(file));
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
