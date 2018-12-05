extern crate num;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::mem::size_of;
use std::path::Path;

use num::PrimInt;

#[derive(Serialize)]
pub enum CompressedStream {
    Gzip(GzipStream),
}

#[derive(Serialize)]
pub struct GzipStream {
    magic: Value<u16>,
    method: Value<u8>,
    flags: Value<u8>,
    time: Value<u32>,
    xflags: Value<u8>,
    os: Value<u8>,
    deflate: DeflateStream,
}

#[derive(Serialize)]
pub struct DeflateStream {
    blocks: Vec<DeflateBlock>,
}

#[derive(Serialize)]
pub struct DeflateBlock {
    bfinal: Value<u8>,
    btype: Value<u8>,
}

#[derive(Serialize)]
struct DataStream {
    bytes: Vec<u8>,
    pos: usize,
    end: usize,
}

impl DataStream {
    fn new(path: &Path) -> Result<DataStream, Error> {
        let mut f = File::open(path)?;
        let len: usize = f.seek(SeekFrom::End(0))? as usize;
        f.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        bytes.resize(len as usize, 0);
        f.read(&mut bytes)?;
        Ok(DataStream { bytes, pos: 0, end: len * 8 })
    }

    fn require(&self, n: usize) -> Result<(), Error> {
        if self.pos + n <= self.end {
            Ok(())
        } else {
            Err(self.parse_error("EOF"))
        }
    }

    fn byte_index(&self) -> Result<usize, Error> {
        if self.pos % 8 == 0 {
            Ok(self.pos / 8)
        } else {
            Err(self.parse_error("Unaligned"))
        }
    }

    fn peek_le<T: PrimInt>(&self) -> Result<Value<T>, Error> {
        let bytes = size_of::<T>();
        self.require(bytes * 8)?;
        let index = self.byte_index()?;
        let mut v = T::zero();
        for i in 0..bytes {
            let b = T::from(self.bytes[index + i]).ok_or(
                ParseError { pos: self.pos, msg: String::from("Conversion") })?;
            v = v | (b << (i * 8));
        }
        Ok(Value {
            v,
            start: self.pos,
            end: self.pos + 8,
        })
    }

    fn pop_le<T: PrimInt>(&mut self) -> Result<Value<T>, Error> {
        let result = self.peek_le::<T>()?;
        self.pos += size_of::<T>() * 8;
        Ok(result)
    }

    fn drop(&mut self, n: usize) -> Result<(), Error> {
        self.require(n)?;
        self.pos += n;
        Ok(())
    }

    fn parse_error(&self, msg: &str) -> Error {
        Error::from(ParseError { pos: self.pos, msg: String::from(msg) })
    }

    fn peek_bits<T: PrimInt>(&mut self, n: usize) -> Result<Value<T>, Error> {
        self.require(n)?;
        let mut v = T::zero();
        for i in 0..n {
            let pos = self.pos + i;
            let b = T::from(self.bytes[pos / 8]).ok_or(
                ParseError { pos: self.pos, msg: String::from("Conversion") })?;
            v = v | (((b >> (pos % 8)) & T::one()) << i);
        }
        Ok(Value {
            v: v,
            start: self.pos,
            end: self.pos + n,
        })
    }

    fn pop_bits<T: PrimInt>(&mut self, n: usize) -> Result<Value<T>, Error> {
        let v = self.peek_bits(n)?;
        self.pos += n;
        Ok(v)
    }
}

#[derive(Serialize)]
pub struct Value<T> {
    v: T,
    start: usize,
    end: usize,
}

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Parse(ParseError),
    Serde(serde_json::Error),
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::Io(error)
    }
}

impl From<ParseError> for Error {
    fn from(error: ParseError) -> Self {
        Error::Parse(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error::Serde(error)
    }
}

#[derive(Debug)]
pub struct ParseError {
    pos: usize,
    msg: String,
}

fn parse_deflate_block(data: &mut DataStream) -> Result<DeflateBlock, Error> {
    let bfinal = data.pop_bits::<u8>(1)?;
    let btype = data.pop_bits::<u8>(2)?;
    Ok(DeflateBlock { bfinal, btype })
}

fn parse_deflate(data: &mut DataStream) -> Result<DeflateStream, Error> {
    let mut deflate = DeflateStream {
        blocks: vec![],
    };
    let block = parse_deflate_block(data)?;
    deflate.blocks.push(block);
    Ok(deflate)
}

pub fn parse(path: &Path) -> Result<CompressedStream, Error> {
    let mut data = DataStream::new(path)?;
    let magic = data.peek_le::<u16>()?;
    if magic.v == 0x8b1f {
        data.drop(16)?;
        let method = data.pop_le::<u8>()?;
        let flags = data.pop_le::<u8>()?;
        let time = data.pop_le::<u32>()?;
        let xflags = data.pop_le::<u8>()?;
        let os = data.pop_le::<u8>()?;
        let deflate = parse_deflate(&mut data)?;
        Ok(CompressedStream::Gzip(GzipStream {
            magic,
            method,
            flags,
            time,
            xflags,
            os,
            deflate,
        }))
    } else {
        Err(data.parse_error("Could not detect stream type"))
    }
}
