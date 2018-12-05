#[derive(Serialize)]
pub enum CompressedStream {
    Gzip(GzipStream),
}

#[derive(Serialize)]
pub struct GzipStream {
    pub magic: Value<u16>,
    pub method: Value<u8>,
    pub flags: Value<u8>,
    pub time: Value<u32>,
    pub xflags: Value<u8>,
    pub os: Value<u8>,
    pub deflate: DeflateStream,
}

#[derive(Serialize)]
pub struct DeflateStream {
    pub blocks: Vec<DeflateBlock>,
}

#[derive(Serialize)]
pub enum DeflateBlock {
    Dynamic(DeflateBlockDynamic),
}

#[derive(Serialize)]
pub struct DeflateBlockDynamic {
    pub header: DeflateBlockHeader,
    pub hlit: Value<u8>,
    pub hdist: Value<u8>,
    pub hclen: Value<u8>,
    pub hclens: Vec<Value<u8>>,
    pub code_length_codes: Vec<HuffmanCode<u8>>,
    pub code_length_tree: HuffmanTree<u8>,
}

#[derive(Serialize)]
pub struct DeflateBlockHeader {
    pub bfinal: Value<u8>,
    pub btype: Value<u8>,
}

#[derive(Serialize)]
pub struct Value<T> {
    pub v: T,
    pub start: usize,
    pub end: usize,
}

#[derive(Serialize)]
pub struct HuffmanCode<T> {
    pub symbol: T,
    pub code: u16,
    pub len: u8,
    pub bin: String,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum HuffmanTree<T> {
    Node(Box<[HuffmanTree<T>; 2]>),
    Leaf(Option<T>),
}
