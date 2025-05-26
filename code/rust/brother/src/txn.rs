use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;
use serde_bytes::ByteBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signed<T>
where
    T: Serialize + DeserializeOwned,
{
    pub conf: String,
    pub data: T,
    pub keys: Vec<ByteBuf>,   // raw bytes already
    pub sigs: Vec<ByteBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Call {
    pub contract: u64,
    pub method:   String,
    pub params:   Vec<(String, String)>,  // no attribute needed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Txn {
    pub nonce: u64,
    pub calls: Vec<Call>,
}

#[async_trait::async_trait]
pub trait ExecutableTxn: Serialize + DeserializeOwned + Send {
    async fn execute(&mut self) -> Result<Vec<(String, ByteBuf)>, u64>;
}
