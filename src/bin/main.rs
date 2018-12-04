extern crate clap;
extern crate deflate_parser;
extern crate serde_json;

use std::path::Path;

use clap::{App, Arg};

use deflate_parser::{Error, parse};

fn main() -> Result<(), Error> {
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(Arg::with_name("INPUT")
            .required(true)
            .index(1))
        .get_matches();
    let input = matches.value_of("INPUT").unwrap();
    let stream = parse(Path::new(input))?;
    println!("{}", serde_json::to_string_pretty(&stream)?);
    Ok(())
}
