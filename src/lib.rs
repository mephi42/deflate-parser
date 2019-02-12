extern crate num;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate sha2;

use std::fmt::Debug;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::mem::size_of;
use std::path::Path;

use num::PrimInt;
use sha2::{Digest, Sha256};

use data::{CompressedStream, DeflateBlock, DeflateBlockDynamic, DeflateBlockExt,
           DeflateBlockFixed, DeflateBlockHeader, DeflateBlockStored, DeflateStream,
           DynamicHuffmanTable, EobToken, GzipStream, HuffmanCode, HuffmanTree, LiteralToken, Token,
           Value, WindowToken, ZlibStream};
use error::{Error, ParseError};

pub mod error;
pub mod data;

impl DataStream {
    fn new(path: &Path, pos: usize) -> Result<DataStream, Error> {
        DataStream::new_from_file(File::open(path)?, pos)
    }

    fn new_from_file(mut f: File, pos: usize) -> Result<DataStream, Error> {
        let len: usize = f.seek(SeekFrom::End(0))? as usize;
        f.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        bytes.resize(len as usize, 0);
        f.read_exact(&mut bytes)?;
        Ok(DataStream { bytes, pos, end: len * 8 })
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

    fn pop_le<'a, T: PrimInt>(&mut self, out: &'a mut Option<Value<T>>)
                              -> Result<&'a Value<T>, Error> {
        *out = Some(self.peek_le::<T>()?);
        self.pos += size_of::<T>() * 8;
        Ok(match out {
            Some(x) => x,
            None => unreachable!()
        })
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
        let bits = match out {
            Some(x) => x,
            None => unreachable!()
        };
        self.pos += n;
        Ok(bits)
    }

    fn align(&mut self) -> Result<(), Error> {
        let n = (8 - (self.pos & 7)) & 7;
        self.drop(n)
    }

    fn pop_bytes(&mut self, out: &mut Option<Value<String>>, n: usize, settings: &Settings)
                 -> Result<(), Error> {
        let index = self.byte_index()?;
        let bits = n * 8;
        self.require(bits)?;
        if settings.data {
            let mut h = Sha256::new();
            h.input(&self.bytes[index..index + n]);
            *out = Some(Value {
                v: format!("sha256:{:x}", h.result()),
                start: self.pos,
                end: self.pos + bits,
            });
        }
        self.pos += bits;
        Ok(())
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
    let hclens = match out {
        Some(x) => x,
        None => unreachable!()
    };
    for _ in 0..n {
        let mut bits: Option<Value<u8>> = None;
        data.pop_bits(&mut bits, 3)?;
        hclens.push(bits.expect("bits"));
    }
    Ok(hclens)
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

fn add_to_huffman_tree<T: Debug>(tree: &mut HuffmanTree<T>, pos: usize,
                                 code: u16, len: usize, symbol: T)
                                 -> Result<(), Error> {
    if len == 0 {
        if tree.is_empty_leaf() {
            *tree = HuffmanTree::Leaf(Some(symbol));
            Ok(())
        } else {
            Err(Error::Parse(ParseError {
                pos,
                msg: format!("Not an empty leaf (symbol={:?})", symbol),
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
            _ => {
                Err(Error::Parse(ParseError {
                    pos,
                    msg: match tree {
                        HuffmanTree::Leaf(Some(old_symbol)) =>
                            format!("Conflict (symbol={:?} and {:?})", old_symbol, symbol),
                        _ =>
                            format!("Conflict (symbol={:?})", symbol),
                    },
                }))
            }
        }
    }
}

fn code_to_bin(out: &mut String, code: u16, len: usize) {
    for i in (0..len).rev() {
        out.push(if (code & (1 << i)) == 0 { '0' } else { '1' });
    }
}

fn build_huffman_codes<T: Clone + Ord>(alphabet: &[T], lens: &[Value<u8>]) -> Vec<HuffmanCode<T>> {
    // 3.2.2. Use of Huffman coding in the "deflate" format
    const MAX_BITS: usize = 15;

    // 1)  Count the number of codes for each code length
    let mut bl_count: [u16; MAX_BITS + 1] = [0; MAX_BITS + 1];
    for len in lens {
        bl_count[len.v as usize] += 1;
    }

    // 2)  Find the numerical value of the smallest code for each code length
    let mut next_code: [u16; MAX_BITS + 1] = [0; MAX_BITS + 1];
    let mut code: u16 = 0;
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

fn build_huffman_tree<'a, T: Clone + Debug>(out: &'a mut Option<HuffmanTree<T>>,
                                            codes: &[HuffmanCode<T>])
                                            -> Result<&'a HuffmanTree<T>, Error> {
    *out = Some(HuffmanTree::Leaf(None));
    let tree = match out {
        Some(ref mut x) => x,
        None => unreachable!()
    };
    for code in codes {
        add_to_huffman_tree(
            tree, code.len.start,
            code.code, code.len.v as usize, code.symbol.clone())?;
    }
    Ok(tree)
}

fn parse_huffman_code<T: Clone>(data: &mut DataStream, tree: &HuffmanTree<T>, start: usize,
                                code: u16, len: usize)
                                -> Result<Value<T>, Error> {
    match tree {
        HuffmanTree::Node(children) => {
            let mut option_bit: Option<Value<usize>> = None;
            let bit = data.pop_bits(&mut option_bit, 1)?;
            parse_huffman_code(data, &children[bit.v], start,
                               (code << 1) | bit.v as u16, len + 1)
        }
        HuffmanTree::Leaf(Some(symbol)) => Ok(Value { v: symbol.clone(), start, end: data.pos }),
        HuffmanTree::Leaf(None) => {
            let mut bin = String::with_capacity(len);
            code_to_bin(&mut bin, code, len);
            Err(data.parse_error(&format!("Code=0b{}", bin)))
        }
    }
}

fn parse_huffman_code_lengths<'a>(out: &'a mut Option<Vec<Value<u8>>>, data: &mut DataStream,
                                  n: usize, tree: &HuffmanTree<u8>)
                                  -> Result<&'a Vec<Value<u8>>, Error> {
    // 3.2.7. Compression with dynamic Huffman codes (BTYPE=10)
    *out = Some(Vec::with_capacity(n));
    let lens = match out {
        Some(x) => x,
        None => unreachable!()
    };
    while lens.len() < n {
        let start = data.pos;
        let value = parse_huffman_code(data, tree, start, 0, 0)?;
        match value.v {
            0...15 => {
                // 0 - 15: Represent code lengths of 0 - 15
                lens.push(value)
            }
            16...18 => {
                let (what, start, repeat_add, repeat_len) = match value.v {
                    // 16: Copy the previous code length 3 - 6 times
                    16 => {
                        let last = lens.last().ok_or_else(
                            || data.parse_error("Repeat"))?;
                        (last.v, last.start, 3, 2)
                    }
                    // 17: Repeat a code length of 0 for 3 - 10 times
                    17 => (0, value.start, 3, 3),
                    // 18: Repeat a code length of 0 for 11 - 138 times
                    18 => (0, value.start, 11, 7),
                    _ => unreachable!()
                };
                let mut option_repeat: Option<Value<usize>> = None;
                let repeat = data.pop_bits(&mut option_repeat, repeat_len)?;
                for _ in 0..(repeat_add + repeat.v) {
                    lens.push(Value {
                        v: what,
                        start,
                        end: repeat.end,
                    });
                }
            }
            _ => return Err(data.parse_error("Code length"))
        }
    }
    if lens.len() == n {
        Ok(lens)
    } else {
        Err(data.parse_error("Code lengths"))
    }
}

fn parse_deflate_block_header(out: &mut Option<DeflateBlockHeader>, data: &mut DataStream)
                              -> Result<(), Error> {
    // 3.2.3. Details of block format
    *out = Some(DeflateBlockHeader { bfinal: None, btype: None });
    let header = match out {
        Some(x) => x,
        None => unreachable!()
    };
    data.pop_bits(&mut header.bfinal, 1)?;
    data.pop_bits(&mut header.btype, 2)?;
    Ok(())
}

fn parse_tokens(out: &mut Option<Vec<Value<Token>>>, data: &mut DataStream, plain_pos: &mut usize,
                hlits_tree: &HuffmanTree<u16>, hdists_tree: &HuffmanTree<u8>, settings: &Settings)
                -> Result<(), Error> {
    // 3.2.5. Compressed blocks (length and distance codes)
    if settings.data {
        *out = Some(Vec::new());
    }
    let mut is_eob = false;
    while !is_eob {
        let start = data.pos;
        let literal = parse_huffman_code(data, hlits_tree, start, 0, 0)?;
        let token_plain_pos = *plain_pos;
        let v = match literal.v {
            0...255 => {
                *plain_pos += 1;
                let v = literal.v as u8;
                Token::Literal(LiteralToken {
                    plain_pos: token_plain_pos,
                    v,
                    c: v as char,
                })
            }
            256 => {
                is_eob = true;
                Token::Eob(EobToken {
                    plain_pos: token_plain_pos,
                })
            }
            257...285 => {
                let literal_extras = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2,
                    3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
                let literal_bases: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23,
                    27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
                let distance_extras = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6,
                    7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
                let distance_bases = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129,
                    193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289,
                    16385, 24577];
                let mut option_literal_extra: Option<Value<u8>> = None;
                let literal_index = literal.v as usize - 257;
                let literal_extra = data.pop_bits(
                    &mut option_literal_extra, literal_extras[literal_index])?;
                let length_value = literal_bases[literal_index] + u16::from(literal_extra.v);
                *plain_pos += length_value as usize;
                let distance_start = data.pos;
                let distance = parse_huffman_code(
                    data, hdists_tree, distance_start, 0, 0)?;
                let mut option_distance_extra: Option<Value<u16>> = None;
                let distance_extra = data.pop_bits(
                    &mut option_distance_extra, distance_extras[distance.v as usize])?;
                let distance_value = distance_bases[distance.v as usize] + distance_extra.v;
                Token::Window(WindowToken {
                    plain_pos: token_plain_pos,
                    length: literal,
                    length_extra: literal_extra.clone(),
                    length_value,
                    distance,
                    distance_extra: distance_extra.clone(),
                    distance_value,
                })
            }
            _ => return Err(data.parse_error("Literal"))
        };
        match out {
            Some(x) => x.push(Value {
                v,
                start,
                end: data.pos,
            }),
            None => {}
        };
    }
    Ok(())
}

fn parse_deflate_block_stored(out: &mut DeflateBlockStored, data: &mut DataStream,
                              plain_pos: &mut usize, settings: &Settings)
                              -> Result<(), Error> {
    // 3.2.4. Non-compressed blocks (BTYPE=00)
    data.align()?;
    let len = data.pop_le(&mut out.len)?;
    let len_usize = len.v as usize;
    data.pop_le(&mut out.nlen)?;
    data.pop_bytes(&mut out.data, len_usize, settings)?;
    *plain_pos += len_usize;
    Ok(())
}

fn parse_deflate_block_fixed(out: &mut DeflateBlockFixed, data: &mut DataStream,
                             plain_pos: &mut usize, settings: &Settings)
                             -> Result<(), Error> {
    // Compression with fixed Huffman codes (BTYPE=01)
    let v5 = Value { v: 5, start: data.pos, end: data.pos };
    let v7 = Value { v: 7, start: data.pos, end: data.pos };
    let v8 = Value { v: 8, start: data.pos, end: data.pos };
    let v9 = Value { v: 9, start: data.pos, end: data.pos };
    let hlits = std::iter::repeat(v8.clone()).take((0u16..=143).len())
        .chain(std::iter::repeat(v9.clone()).take((144u16..=255).len()))
        .chain(std::iter::repeat(v7.clone()).take((256u16..=279).len()))
        .chain(std::iter::repeat(v8.clone()).take((280u16..=287).len()))
        .collect::<Vec<Value<u8>>>();
    let hlits_codes = build_huffman_codes(
        &(0..=285).collect::<Vec<u16>>(), &hlits);
    let mut option_hlits_tree: Option<HuffmanTree<u16>> = None;
    let hlits_tree = build_huffman_tree(
        &mut option_hlits_tree, &hlits_codes)?;
    let hdists = std::iter::repeat(v5.clone()).take((0u8..=31).len())
        .collect::<Vec<Value<u8>>>();
    let hdists_codes = build_huffman_codes(
        &(0..=31).collect::<Vec<u8>>(), &hdists);
    let mut option_hdists_tree: Option<HuffmanTree<u8>> = None;
    let hdists_tree = build_huffman_tree(
        &mut option_hdists_tree, &hdists_codes)?;
    parse_tokens(&mut out.tokens, data, plain_pos, &hlits_tree, &hdists_tree, settings)?;
    Ok(())
}

fn parse_dht(out: &mut DynamicHuffmanTable, data: &mut DataStream)
             -> Result<(), Error> {
    // 3.2.7. Compression with dynamic Huffman codes (BTYPE=10)
    // 5 Bits: HLIT, # of Literal/Length codes - 257 (257 - 286)
    let hlit = data.pop_bits(&mut out.hlit, 5)?;
    if hlit.v > 29 {
        return Err(data.parse_error("HLIT > 29"));
    }
    // 5 Bits: HDIST, # of Distance codes - 1        (1 - 32)
    let hdist = data.pop_bits(&mut out.hdist, 5)?;
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
    // HDIST + 1 code lengths for the distance alphabet
    let hlits_count = (hlit.v as usize) + 257;
    let hdists_count = (hdist.v as usize) + 1;
    let hlits_hdists = parse_huffman_code_lengths(
        &mut out.hlits, data, hlits_count + hdists_count, &hclens_tree)?;
    out.hlits_codes = Some(build_huffman_codes(
        &(0..=285).collect::<Vec<u16>>(), &hlits_hdists[..hlits_count]));
    match &out.hlits_codes {
        Some(hlits_codes) => build_huffman_tree(
            &mut out.hlits_tree, &hlits_codes)?,
        None => unreachable!()
    };
    out.hdists_codes = Some(build_huffman_codes(
        &(0..=29).collect::<Vec<u8>>(), &hlits_hdists[hlits_count..]));
    match &out.hdists_codes {
        Some(hdists_codes) => build_huffman_tree(&mut out.hdists_tree, &hdists_codes)?,
        None => unreachable!()
    };
    Ok(())
}

fn parse_deflate_block_dynamic(out: &mut DeflateBlockDynamic, data: &mut DataStream,
                               plain_pos: &mut usize, settings: &Settings)
                               -> Result<(), Error> {
    out.dht = Some(DynamicHuffmanTable::default());
    let dht = match &mut out.dht {
        Some(x) => x,
        None => unreachable!()
    };
    parse_dht(dht, data)?;
    let hlits_tree = match &dht.hlits_tree {
        Some(x) => x,
        None => unreachable!()
    };
    let hdists_tree = match &dht.hdists_tree {
        Some(x) => x,
        None => unreachable!()
    };
    // The actual compressed data of the block
    // The literal/length symbol
    parse_tokens(&mut out.tokens, data, plain_pos, &hlits_tree, &hdists_tree, settings)?;
    Ok(())
}

fn parse_deflate_block(out: &mut Vec<DeflateBlock>, data: &mut DataStream, plain_pos: &mut usize,
                       settings: &Settings)
                       -> Result<bool, Error> {
    let mut option_header: Option<DeflateBlockHeader> = None;
    parse_deflate_block_header(&mut option_header, data)?;
    out.push(DeflateBlock {
        header: match option_header {
            Some(x) => x,
            None => unreachable!()
        },
        plain_start: Some(*plain_pos),
        plain_end: None,
        ext: None,
    });
    let block = match out.last_mut() {
        Some(x) => x,
        None => unreachable!()
    };
    let bfinal = match &block.header.bfinal {
        Some(x) => x.v == 1,
        _ => unreachable!()
    };
    let btype = match &block.header.btype {
        Some(btype) => btype.v,
        None => unreachable!()
    };
    match btype {
        0 => {
            block.ext = Some(DeflateBlockExt::Stored(DeflateBlockStored {
                len: None,
                nlen: None,
                data: None,
            }));
            let ext = match block.ext {
                Some(DeflateBlockExt::Stored(ref mut x)) => x,
                _ => unreachable!()
            };
            parse_deflate_block_stored(ext, data, plain_pos, settings)?;
        }
        1 => {
            block.ext = Some(DeflateBlockExt::Fixed(DeflateBlockFixed {
                tokens: None,
            }));
            let ext = match block.ext {
                Some(DeflateBlockExt::Fixed(ref mut x)) => x,
                _ => unreachable!()
            };
            parse_deflate_block_fixed(ext, data, plain_pos, settings)?;
        }
        2 => {
            block.ext = Some(DeflateBlockExt::Dynamic(Box::new(DeflateBlockDynamic {
                dht: None,
                tokens: None,
            })));
            let ext = match block.ext {
                Some(DeflateBlockExt::Dynamic(ref mut x)) => x,
                _ => unreachable!()
            };
            parse_deflate_block_dynamic(ext, data, plain_pos, settings)?;
        }
        _ => return Err(data.parse_error(&format!("BTYPE={}", btype)))
    }
    block.plain_end = Some(*plain_pos);
    Ok(!bfinal)
}

fn parse_deflate(deflate: &mut DeflateStream, data: &mut DataStream, settings: &Settings)
                 -> Result<(), Error> {
    let mut plain_pos: usize = 0;
    while parse_deflate_block(&mut deflate.blocks, data, &mut plain_pos, settings)? {}
    Ok(())
}

fn parse_zlib(zlib: &mut ZlibStream, data: &mut DataStream, settings: &Settings) -> Result<(), Error> {
    data.pop_le(&mut zlib.cmf)?;
    data.pop_le(&mut zlib.flg)?;
    zlib.deflate = Some(DeflateStream::default());
    match &mut zlib.deflate {
        Some(deflate) => parse_deflate(deflate, data, settings)?,
        None => unreachable!()
    }
    data.align()?;
    data.pop_le(&mut zlib.adler32)?;
    Ok(())
}

fn parse_gzip(out: &mut Option<CompressedStream>, data: &mut DataStream, settings: &Settings)
              -> Result<(), Error> {
    let magic = data.peek_le::<u16>()?;
    if magic.v == 0x8b1f {
        data.drop(16)?;
        *out = Some(CompressedStream::Gzip(Box::new(GzipStream {
            magic,
            method: None,
            flags: None,
            time: None,
            xflags: None,
            os: None,
            deflate: None,
            checksum: None,
            len: None,
        })));
        let gzip = match out {
            Some(CompressedStream::Gzip(x)) => x,
            _ => unreachable!()
        };
        data.pop_le(&mut gzip.method)?;
        data.pop_le(&mut gzip.flags)?;
        data.pop_le(&mut gzip.time)?;
        data.pop_le(&mut gzip.xflags)?;
        data.pop_le(&mut gzip.os)?;
        gzip.deflate = Some(DeflateStream::default());
        match &mut gzip.deflate {
            Some(deflate) => parse_deflate(deflate, data, settings)?,
            None => unreachable!()
        }
        data.align()?;
        data.pop_le(&mut gzip.checksum)?;
        data.pop_le(&mut gzip.len)?;
        Ok(())
    } else {
        Err(data.parse_error("Stream type"))
    }
}

fn parse_data_stream(out: &mut Option<CompressedStream>, mut data: DataStream, settings: &Settings)
                     -> Result<(), Error> {
    match out {
        Some(CompressedStream::Raw(deflate)) =>
            parse_deflate(deflate, &mut data, settings),
        Some(CompressedStream::Dht(dht)) =>
            parse_dht(dht, &mut data),
        Some(CompressedStream::Zlib(zlib)) =>
            parse_zlib(zlib, &mut data, settings),
        _ =>
            parse_gzip(out, &mut data, settings),
    }?;
    if data.pos == data.end {
        Ok(())
    } else {
        Err(data.parse_error(&format!("Garbage (end={})", data.end)))
    }
}

pub struct Settings {
    pub bit_offset: usize,
    pub data: bool,
}

pub fn parse(out: &mut Option<CompressedStream>, path: &Path, settings: &Settings)
             -> Result<(), Error> {
    let data = DataStream::new(path, settings.bit_offset)?;
    parse_data_stream(out, data, settings)
}

pub fn parse_file(out: &mut Option<CompressedStream>, file: File, settings: &Settings)
                  -> Result<(), Error> {
    let data = DataStream::new_from_file(file, settings.bit_offset)?;
    parse_data_stream(out, data, settings)
}
