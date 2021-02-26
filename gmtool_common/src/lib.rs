use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum FileEntry {
    GCSFile(String),
    DirectoryEntry(String),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum GCSAgentMessage {
    FileChange(String),
    FileList(Vec<FileEntry>),
}
