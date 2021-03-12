#![allow(clippy::single_component_path_imports, clippy::large_enum_variant)]
use gcs;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum FileEntry {
    GCSFile(String),
    Directory(String),
}

mod as_cbor {
    use serde::de::{Deserialize, DeserializeOwned, Deserializer};
    use serde::ser::{Serialize, Serializer};
    use serde_cbor;

    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Serialize,
        S: Serializer,
    {
        use serde::ser::Error;
        let v = serde_cbor::to_vec(value).map_err(Error::custom)?;
        v.serialize(serializer)
    }

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: DeserializeOwned,
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        let v = Vec::<u8>::deserialize(deserializer)?;
        serde_cbor::from_slice(&v).map_err(Error::custom)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GCSFile {
    pub path: String,
    #[serde(with = "as_cbor")]
    pub file: gcs::FileKind,
}

/// Messages from the GCS Agent to the Web UI
#[derive(Serialize, Deserialize, Debug)]
pub enum GCSAgentMessage {
    /// One sent per RequestChDir
    RequestChDirResult(Result<(), String>),
    /// One sent per RequestSheetContents
    RequestSheetContentsResult(Result<GCSFile, String>),
    /// One sent per RequestWatch
    RequestWatchResult(Result<(), String>),
    // TODO: should this be oneshot notifications?
    /// Some number sent while watching
    FileChangeNotification(String),
    /// One sent on start unless no past config and current doesn't exist,
    /// one sent on successful RequestChDir
    /// File entries are all directories that are parsable as unicode
    /// and all files that are unicode that ends with gcs.
    DirectoryChangeNotification(Result<(String, Vec<FileEntry>), String>),
}

/// Messages from the Web UI to the GCS Agent
#[derive(Serialize, Deserialize, Debug)]
pub enum WebUIMessage {
    RequestChDir(String),
    RequestSheetContents(String),
    RequestWatch(String),
}
