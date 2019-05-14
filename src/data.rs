#[derive(Serialize)]
#[serde(untagged)]
pub enum CompressedStream {
    Raw(DeflateStream),
    Gzip(Box<GzipStream>),
    Dht(Box<DynamicHuffmanTable>),
    Zlib(ZlibStream),
}

#[derive(Serialize)]
pub struct GzipStream {
    pub magic: Value<u16>,
    pub method: Option<Value<u8>>,
    pub flags: Option<Value<u8>>,
    pub time: Option<Value<u32>>,
    pub xflags: Option<Value<u8>>,
    pub os: Option<Value<u8>>,
    pub name: Option<Value<String>>,
    pub deflate: Option<DeflateStream>,
    pub checksum: Option<Value<u32>>,
    pub len: Option<Value<u32>>,
}

#[derive(Default, Serialize)]
pub struct ZlibStream {
    pub cmf: Option<Value<u8>>,
    pub flg: Option<Value<u8>>,
    pub dictid: Option<Value<u32>>,
    pub deflate: Option<DeflateStream>,
    pub adler32: Option<Value<u32>>,
}

#[derive(Default, Serialize)]
pub struct DeflateStream {
    pub blocks: Vec<DeflateBlock>,
}

#[derive(Serialize)]
pub struct DeflateBlock {
    pub header: DeflateBlockHeader,
    pub end: Option<usize>,
    pub plain_start: Option<usize>,
    pub plain_end: Option<usize>,
    #[serde(flatten)]
    pub ext: Option<DeflateBlockExt>,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum DeflateBlockExt {
    Stored(DeflateBlockStored),
    Fixed(DeflateBlockFixed),
    Dynamic(Box<DeflateBlockDynamic>),
}

#[derive(Serialize)]
pub struct DeflateBlockStored {
    pub len: Option<Value<u16>>,
    pub nlen: Option<Value<u16>>,
    pub data: Option<Value<String>>,
}

#[derive(Serialize)]
pub struct DeflateBlockFixed {
    pub tokens: Option<Vec<Value<Token>>>,
}

#[derive(Default, Serialize)]
pub struct DynamicHuffmanTable {
    pub hlit: Option<Value<u8>>,
    pub hdist: Option<Value<u8>>,
    pub hclen: Option<Value<u8>>,
    pub hclens: Option<Vec<Value<u8>>>,
    pub hclens_codes: Option<Vec<HuffmanCode<u8>>>,
    pub hclens_tree: Option<HuffmanTree<u8>>,
    pub hlits: Option<Vec<Value<u8>>>,
    pub hlits_codes: Option<Vec<HuffmanCode<u16>>>,
    pub hlits_tree: Option<HuffmanTree<u16>>,
    pub hdists: Option<Vec<Value<u8>>>,
    pub hdists_codes: Option<Vec<HuffmanCode<u8>>>,
    pub hdists_tree: Option<HuffmanTree<u8>>,
}

#[derive(Serialize)]
pub struct DeflateBlockDynamic {
    pub dht: Option<DynamicHuffmanTable>,
    pub tokens: Option<Vec<Value<Token>>>,
}

#[derive(Serialize)]
pub struct DeflateBlockHeader {
    pub bfinal: Option<Value<u8>>,
    pub btype: Option<Value<u8>>,
}

#[derive(Clone, Serialize)]
pub struct Value<T: Clone> {
    pub v: T,
    pub start: usize,
    pub end: usize,  // non-inclusive
}

#[derive(Serialize)]
pub struct HuffmanCode<T> {
    pub symbol: T,
    pub code: u16,
    pub len: Value<u8>,
    pub bin: String,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum HuffmanTree<T> {
    Node(Box<[HuffmanTree<T>; 2]>),
    Leaf(Option<T>),
}

#[derive(Clone, Serialize)]
pub struct LiteralToken {
    pub plain_pos: usize,
    pub v: u8,
    pub c: char,
}

#[derive(Clone, Serialize)]
pub struct EobToken {
    pub plain_pos: usize,
}

#[derive(Clone, Serialize)]
pub struct WindowToken {
    pub plain_pos: usize,
    pub length: Value<u16>,
    pub length_extra: Value<u8>,
    pub length_value: u16,
    pub distance: Value<u8>,
    pub distance_extra: Value<u16>,
    pub distance_value: u16,
}

#[derive(Clone, Serialize)]
#[serde(untagged)]
pub enum Token {
    Literal(LiteralToken),
    Eob(EobToken),
    Window(WindowToken),
}
