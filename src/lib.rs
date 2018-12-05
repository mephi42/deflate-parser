extern crate num;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::mem::size_of;
use std::path::Path;

use num::PrimInt;

use data::{CompressedStream, DeflateBlock, DeflateBlockDynamic, DeflateBlockHeader, DeflateStream,
           GzipStream, HuffmanCode, HuffmanTree, Value};
use error::{Error, HuffmanTreeError, ParseError};

pub mod error;
pub mod data;

impl DataStream {
    fn new(path: &Path) -> Result<DataStream, Error> {
        let mut f = File::open(path)?;
        let len: usize = f.seek(SeekFrom::End(0))? as usize;
        f.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        bytes.resize(len as usize, 0);
        f.read_exact(&mut bytes)?;
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
            v,
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

struct DataStream {
    bytes: Vec<u8>,
    pos: usize,
    end: usize,
}

fn parse_hclens(hclen: u8, data: &mut DataStream) -> Result<Vec<Value<u8>>, Error> {
    let n = (hclen + 4) as usize;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        v.push(data.pop_bits(3)?);
    }
    Ok(v)
}

impl<T> HuffmanTree<T> {
    fn is_empty_leaf(&self) -> bool {
        match self {
            HuffmanTree::Leaf(None) => true,
            _ => false,
        }
    }

    fn new_node() -> HuffmanTree<T> {
        HuffmanTree::Node(Box::new([HuffmanTree::Leaf(None), HuffmanTree::Leaf(None)]))
    }
}

fn add_to_huffman_tree<T>(tree: &mut HuffmanTree<T>, code: u16, len: usize, symbol: T)
                          -> Result<(), String> {
    if len == 0 {
        if tree.is_empty_leaf() {
            *tree = HuffmanTree::Leaf(Some(symbol));
            Ok(())
        } else {
            Err(String::from("Not an empty leaf"))
        }
    } else {
        if tree.is_empty_leaf() {
            *tree = HuffmanTree::new_node();
        }
        let bit = ((code >> (len - 1)) & 1) as usize;
        match tree {
            HuffmanTree::Node(node) => add_to_huffman_tree(
                &mut node[bit], code, len - 1, symbol),
            _ => Err(String::from("Not a node")),
        }
    }
}

fn code_to_string(code: u16, len: usize) -> String {
    let mut s = String::with_capacity(len);
    for i in (0..len).rev() {
        s.push(if (code & (1 << i)) == 0 { '0' } else { '1' });
    }
    s
}

fn build_huffman_codes<T: Clone>(alphabet: &[T], lens: &[u8]) -> Vec<HuffmanCode<T>> {
    // 3.2.2. Use of Huffman coding in the "deflate" format
    const MAX_BITS: usize = 15;

    // 1)  Count the number of codes for each code length
    let mut bl_count: [u16; MAX_BITS + 1] = [0; MAX_BITS + 1];
    for len in lens {
        bl_count[*len as usize] += 1;
    }

    // 2)  Find the numerical value of the smallest code for each code length
    let mut next_code: [u16; MAX_BITS + 1] = [0; MAX_BITS + 1];
    let mut code: u16 = 0;
    for bits in 1..=MAX_BITS {
        code = (code + bl_count[bits - 1]) << 1;
        next_code[bits] = code;
    }

    // 3)  Assign numerical values to all codes
    let mut codes: Vec<HuffmanCode<T>> = Vec::with_capacity(alphabet.len());
    for i in 0..alphabet.len() {
        let len = lens[i];
        if len != 0 {
            let len_index = len as usize;
            codes.push(HuffmanCode {
                symbol: alphabet[i].clone(),
                code: next_code[len_index],
                len,
                bin: code_to_string(next_code[len_index], len_index),
            });
            next_code[len_index] += 1;
        }
    }
    codes
}

fn build_huffman_tree<T: Clone>(codes: &[HuffmanCode<T>])
                                -> Result<HuffmanTree<T>, (HuffmanTree<T>, String)> {
    let mut tree: HuffmanTree<T> = HuffmanTree::Leaf(None);
    for code in codes {
        match add_to_huffman_tree(&mut tree, code.code, code.len as usize, code.symbol.clone()) {
            Ok(()) => {}
            Err(msg) => return Err((tree, msg)),
        }
    }
    Ok(tree)
}

fn parse_deflate_block(data: &mut DataStream) -> Result<DeflateBlock, Error> {
    let bfinal = data.pop_bits::<u8>(1)?;
    let btype = data.pop_bits::<u8>(2)?;
    let header = DeflateBlockHeader { bfinal, btype };
    match header.btype.v {
        2 => {
            let hlit = data.pop_bits::<u8>(5)?;
            let hdist = data.pop_bits::<u8>(5)?;
            let hclen = data.pop_bits::<u8>(4)?;
            let hclens = parse_hclens(hclen.v, data)?;
            let hclens_codes = build_huffman_codes(
                &[16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15],
                &hclens.iter().map(|x| x.v).collect::<Vec<u8>>());
            let hclens_tree: HuffmanTree<u8> = match build_huffman_tree(&hclens_codes) {
                Ok(tree) => tree,
                Err((tree, msg)) => return Err(Error::HuffmanCodeLengths(HuffmanTreeError {
                    tree,
                    codes: hclens_codes,
                    msg,
                })),
            };
            Ok(DeflateBlock::Dynamic(DeflateBlockDynamic {
                header,
                hlit,
                hdist,
                hclen,
                hclens,
                code_length_codes: hclens_codes,
                code_length_tree: hclens_tree,
            }))
        }
        _ => Err(data.parse_error("BTYPE")),
    }
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
