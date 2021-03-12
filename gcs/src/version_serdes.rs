use serde::{Deserialize, Serialize};
use serde_value::Value as SerdeValue;

#[derive(Serialize)]
pub struct VersionSerializeWrapper<T> {
    pub version: u64,
    #[serde(flatten)]
    pub inner: T,
}
#[derive(Deserialize)]
pub struct VersionDeserializeWrapper {
    pub version: u64,
    #[serde(flatten)]
    pub inner: SerdeValue,
}
