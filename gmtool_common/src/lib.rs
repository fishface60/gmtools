use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum FileEntry {
    GCSFile(String),
    Directory(String),
}

/// Messages from the GCS Agent to the Web UI
#[derive(Serialize, Deserialize, Debug)]
pub enum GCSAgentMessage {
    /// One sent per RequestChDir
    RequestChDirResult(Result<(), String>),
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
    RequestWatch(String),
}
