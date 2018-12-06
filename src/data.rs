#[derive(Serialize)]
#[serde(untagged)]
pub enum CompressedStream {
    Gzip(GzipStream),
}

#[derive(Serialize)]
pub struct GzipStream {
    pub magic: Value<u16>,
    pub method: Option<Value<u8>>,
    pub flags: Option<Value<u8>>,
    pub time: Option<Value<u32>>,
    pub xflags: Option<Value<u8>>,
    pub os: Option<Value<u8>>,
    pub deflate: Option<DeflateStream>,
}

#[derive(Serialize)]
pub struct DeflateStream {
    pub blocks: Vec<DeflateBlock>,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum DeflateBlock {
    Stored(DeflateBlockStored),
    Fixed(DeflateBlockFixed),
    Dynamic(DeflateBlockDynamic),
}

#[derive(Serialize)]
pub struct DeflateBlockStored {
    pub header: DeflateBlockHeader,
    pub len: Option<Value<u16>>,
    pub nlen: Option<Value<u16>>,
    pub data: Option<Value<String>>,
}

#[derive(Serialize)]
pub struct DeflateBlockFixed {
    pub header: DeflateBlockHeader,
    pub tokens: Option<Vec<Value<Token>>>,
}

#[derive(Serialize)]
pub struct DeflateBlockDynamic {
    pub header: DeflateBlockHeader,
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
    pub end: usize,
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
#[serde(untagged)]
pub enum Token {
    Literal(u8),
    Eob,
    Window(u16, u8),
}
