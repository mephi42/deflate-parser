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
use error::{Error, ParseError};

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
            let b = T::from(self.bytes[index + i])
                .ok_or_else(|| self.parse_error("Conversion"))?;
            v = v | (b << (i * 8));
        }
        Ok(Value {
            v,
            start: self.pos,
            end: self.pos + 8,
        })
    }

    fn pop_le<T: PrimInt>(&mut self, out: &mut Option<Value<T>>) -> Result<(), Error> {
        *out = Some(self.peek_le::<T>()?);
        self.pos += size_of::<T>() * 8;
        Ok(())
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
            let b = T::from(self.bytes[pos / 8])
                .ok_or_else(|| self.parse_error("Conversion"))?;
            v = v | (((b >> (pos % 8)) & T::one()) << i);
        }
        Ok(Value {
            v,
            start: self.pos,
            end: self.pos + n,
        })
    }

    fn pop_bits<'a, T: PrimInt>(&mut self, out: &'a mut Option<Value<T>>, n: usize)
                                -> Result<&'a Value<T>, Error> {
        *out = Some(self.peek_bits(n)?);
        match out {
            Some(bits) => {
                self.pos += n;
                Ok(bits)
            }
            None => unreachable!()
        }
    }
}

struct DataStream {
    bytes: Vec<u8>,
    pos: usize,
    end: usize,
}

fn parse_hclens<'a>(out: &'a mut Option<Vec<Value<u8>>>, data: &mut DataStream, hclen: u8)
                    -> Result<&'a Vec<Value<u8>>, Error> {
    let n = (hclen + 4) as usize;
    *out = Some(Vec::with_capacity(n));
    match out {
        Some(hclens) => {
            for _ in 0..n {
                let mut bits: Option<Value<u8>> = None;
                data.pop_bits(&mut bits, 3)?;
                hclens.push(bits.expect("bits"));
            }
            Ok(hclens)
        }
        None => unreachable!()
    }
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

fn add_to_huffman_tree<T>(tree: &mut HuffmanTree<T>, pos: usize, code: u32, len: usize, symbol: T)
                          -> Result<(), Error> {
    if len == 0 {
        if tree.is_empty_leaf() {
            *tree = HuffmanTree::Leaf(Some(symbol));
            Ok(())
        } else {
            Err(Error::Parse(ParseError {
                pos,
                msg: String::from("Not an empty leaf"),
            }))
        }
    } else {
        if tree.is_empty_leaf() {
            *tree = HuffmanTree::new_node();
        }
        let bit = ((code >> (len - 1)) & 1) as usize;
        match tree {
            HuffmanTree::Node(children) => add_to_huffman_tree(
                &mut children[bit], pos + 1, code, len - 1, symbol),
            _ => Err(Error::Parse(ParseError {
                pos,
                msg: String::from("Not a node"),
            })),
        }
    }
}

fn code_to_bin(out: &mut String, code: u32, len: usize) {
    for i in (0..len).rev() {
        out.push(if (code & (1 << i)) == 0 { '0' } else { '1' });
    }
}

fn build_huffman_codes<T: Clone + Ord>(alphabet: &[T], lens: &[Value<u8>]) -> Vec<HuffmanCode<T>> {
    // 3.2.2. Use of Huffman coding in the "deflate" format
    const MAX_BITS: usize = 18;

    // 1)  Count the number of codes for each code length
    let mut bl_count: [u32; MAX_BITS + 1] = [0; MAX_BITS + 1];
    for len in lens {
        bl_count[len.v as usize] += 1;
    }

    // 2)  Find the numerical value of the smallest code for each code length
    let mut next_code: [u32; MAX_BITS + 1] = [0; MAX_BITS + 1];
    let mut code: u32 = 0;
    bl_count[0] = 0;
    for bits in 1..=MAX_BITS {
        code = (code + bl_count[bits - 1]) << 1;
        next_code[bits] = code;
    }

    // 3)  Assign numerical values to all codes
    let mut codes: Vec<HuffmanCode<T>> = Vec::with_capacity(alphabet.len());
    for i in 0..alphabet.len() {
        if i < lens.len() {
            let len = lens[i].clone();
            let len_u8 = len.v;
            if len_u8 != 0 {
                codes.push(HuffmanCode {
                    symbol: alphabet[i].clone(),
                    code: 0,
                    len,
                    bin: String::with_capacity(len_u8 as usize),
                });
            }
        }
    }
    codes.sort_by_key(|c| c.symbol.clone());
    for code in &mut codes {
        let len_usize = code.len.v as usize;
        code.code = next_code[len_usize];
        code_to_bin(&mut code.bin, next_code[len_usize], len_usize);
        next_code[len_usize] += 1;
    }
    codes
}

fn build_huffman_tree<'a, T: Clone>(out: &'a mut Option<HuffmanTree<T>>, codes: &[HuffmanCode<T>])
                                    -> Result<&'a HuffmanTree<T>, Error> {
    *out = Some(HuffmanTree::Leaf(None));
    match out {
        Some(ref mut tree) => {
            for code in codes {
                add_to_huffman_tree(
                    tree, code.len.start,
                    code.code, code.len.v as usize, code.symbol.clone())?;
            }
            Ok(tree)
        }
        None => unreachable!()
    }
}

fn parse_huffman_code<T: Clone>(data: &mut DataStream, tree: &HuffmanTree<T>, start: usize)
                                -> Result<Value<T>, Error> {
    match tree {
        HuffmanTree::Node(children) => {
            let mut option_bit: Option<Value<usize>> = None;
            let bit = data.pop_bits(&mut option_bit, 1)?;
            parse_huffman_code(data, &children[bit.v], start)
        }
        HuffmanTree::Leaf(Some(symbol)) => Ok(Value { v: symbol.clone(), start, end: data.pos }),
        HuffmanTree::Leaf(None) => Err(data.parse_error("Code")),
    }
}

fn parse_code_lengths(data: &mut DataStream, n: usize, tree: &HuffmanTree<u8>)
                      -> Result<Vec<Value<u8>>, Error> {
    // 3.2.7. Compression with dynamic Huffman codes (BTYPE=10)
    let mut result: Vec<Value<u8>> = Vec::with_capacity(n);
    while result.len() < n {
        let start = data.pos;
        let value = parse_huffman_code(data, tree, start)?;
        match value.v {
            0...15 => {
                // 0 - 15: Represent code lengths of 0 - 15
                result.push(value)
            }
            16...18 => {
                let (what, repeat_add, repeat_len) = match value.v {
                    // 16: Copy the previous code length 3 - 6 times
                    16 => (result.last().ok_or_else(|| data.parse_error("Repeat"))?.v, 3, 2),
                    // 17: Repeat a code length of 0 for 3 - 10 times
                    17 => (0, 3, 3),
                    // 18: Repeat a code length of 0 for 11 - 138 times
                    18 => (0, 11, 7),
                    _ => unreachable!()
                };
                let mut option_repeat: Option<Value<usize>> = None;
                let repeat = data.pop_bits(&mut option_repeat, repeat_len)?;
                for _ in 0..(repeat_add + repeat.v) {
                    result.push(Value {
                        v: what,
                        start: value.start,
                        end: repeat.end,
                    });
                }
            }
            _ => return Err(data.parse_error("Code length"))
        }
    }
    Ok(result)
}

fn parse_deflate_block_header(out: &mut Option<DeflateBlockHeader>, data: &mut DataStream)
                              -> Result<(), Error> {
    // 3.2.3. Details of block format
    *out = Some(DeflateBlockHeader { bfinal: None, btype: None });
    match out {
        Some(header) => {
            data.pop_bits(&mut header.bfinal, 1)?;
            data.pop_bits(&mut header.btype, 2)?;
            Ok(())
        }
        None => unreachable!()
    }
}

fn parse_deflate_block_dynamic(out: &mut DeflateBlockDynamic, data: &mut DataStream)
                               -> Result<(), Error> {
    // 3.2.7. Compression with dynamic Huffman codes (BTYPE=10)
    // 5 Bits: HLIT, # of Literal/Length codes - 257 (257 - 286)
    data.pop_bits(&mut out.hlit, 5)?;
    // 5 Bits: HDIST, # of Distance codes - 1        (1 - 32)
    data.pop_bits(&mut out.hdist, 5)?;
    // 4 Bits: HCLEN, # of Code Length codes - 4     (4 - 19)
    let hclen = data.pop_bits(&mut out.hclen, 4)?;
    // (HCLEN + 4) x 3 bits: code lengths for the code length alphabet
    let hclens = parse_hclens(
        &mut out.hclens, data, hclen.v)?;
    out.hclens_codes = Some(build_huffman_codes(
        &[16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15],
        &hclens));
    let hclens_tree = match &out.hclens_codes {
        Some(hclens_codes) => build_huffman_tree(
            &mut out.hclens_tree, &hclens_codes)?,
        None => unreachable!()
    };
    // HLIT + 257 code lengths for the literal/length alphabet
    match &out.hlit {
        Some(hlit) => out.hlits = Some(parse_code_lengths(
            data, (hlit.v as usize) + 257, &hclens_tree)?),
        None => unreachable!()
    }
    match &out.hlits {
        Some(hlits) => out.hlits_codes = Some(build_huffman_codes(
            &(0..=285).collect::<Vec<u16>>(), &hlits)),
        None => unreachable!()
    }
    let _hlits_tree = match &out.hlits_codes {
        Some(hlits_codes) => build_huffman_tree(
            &mut out.hlits_tree, &hlits_codes)?,
        None => unreachable!()
    };
    Ok(())
}

fn parse_deflate_block(out: &mut Vec<DeflateBlock>, data: &mut DataStream) -> Result<(), Error> {
    let mut option_header: Option<DeflateBlockHeader> = None;
    parse_deflate_block_header(&mut option_header, data)?;
    match option_header {
        Some(header) => {
            let btype = match &header.btype {
                Some(btype) => btype.v,
                None => unreachable!()
            };
            match btype {
                2 => {
                    out.push(DeflateBlock::Dynamic(DeflateBlockDynamic {
                        header,
                        hlit: None,
                        hdist: None,
                        hclen: None,
                        hclens: None,
                        hclens_codes: None,
                        hclens_tree: None,
                        hlits: None,
                        hlits_codes: None,
                        hlits_tree: None,
                    }));
                    match out.last_mut() {
                        Some(DeflateBlock::Dynamic(ref mut block)) => {
                            parse_deflate_block_dynamic(block, data)?;
                            Ok(())
                        }
                        _ => unreachable!()
                    }
                }
                _ => Err(data.parse_error("BTYPE"))
            }
        }
        None => unreachable!()
    }
}

fn parse_deflate(out: &mut Option<DeflateStream>, data: &mut DataStream) -> Result<(), Error> {
    *out = Some(DeflateStream {
        blocks: Vec::new(),
    });
    match out {
        Some(deflate) => {
            parse_deflate_block(&mut deflate.blocks, data)?;
            Ok(())
        }
        None => unreachable!()
    }
}

pub fn parse(out: &mut Option<CompressedStream>, path: &Path) -> Result<(), Error> {
    let mut data = DataStream::new(path)?;
    let magic = data.peek_le::<u16>()?;
    if magic.v == 0x8b1f {
        data.drop(16)?;
        *out = Some(CompressedStream::Gzip(GzipStream {
            magic,
            method: None,
            flags: None,
            time: None,
            xflags: None,
            os: None,
            deflate: None,
        }));
        match out {
            Some(CompressedStream::Gzip(gzip)) => {
                data.pop_le(&mut gzip.method)?;
                data.pop_le(&mut gzip.flags)?;
                data.pop_le(&mut gzip.time)?;
                data.pop_le(&mut gzip.xflags)?;
                data.pop_le(&mut gzip.os)?;
                parse_deflate(&mut gzip.deflate, &mut data)?;
                Ok(())
            }
            None => unreachable!()
        }
    } else {
        Err(data.parse_error("Stream type"))
    }
}
