use anyhow::Result;

pub trait Codec {
    fn encode(&self, data: Vec<Vec<u8>>) -> Result<Vec<Vec<u8>>>;
    fn decode(&self, data: Vec<Vec<u8>>) -> Result<Vec<Vec<u8>>>;
}
