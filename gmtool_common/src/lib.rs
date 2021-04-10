#![allow(
    clippy::large_enum_variant,
    clippy::single_component_path_imports,
    clippy::upper_case_acronyms
)]
use std::{convert::TryFrom, ffi::OsString, path::PathBuf};

use serde::{Deserialize, Serialize};

use gcs;

#[derive(
    Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub enum PortableOsString {
    Undifferentiated(String),
    Unix { data: Vec<u8>, text: String },
    Windows { data: Vec<u16>, text: String },
}

impl PortableOsString {
    pub fn to_str_lossy(&self) -> &str {
        match self {
            Self::Undifferentiated(s) => s,
            Self::Unix { text, .. } => text,
            Self::Windows { text, .. } => text,
        }
    }
    pub fn to_string_lossy(&self) -> String {
        String::from(self.to_str_lossy())
    }
}

#[cfg(any(unix, windows))]
impl From<OsString> for PortableOsString {
    #[cfg(unix)]
    fn from(s: OsString) -> Self {
        use std::os::unix::ffi::OsStringExt;
        let text: String = s.to_string_lossy().into();
        Self::Unix {
            data: s.into_vec(),
            text,
        }
    }
    #[cfg(windows)]
    fn from(s: OsString) -> Self {
        use std::os::windows::ffi::OsStringExt;
        let text: String = s.to_string_lossy().into();
        Self::Windows {
            data: s.encode_wide().collect(),
            text,
        }
    }
}

#[cfg(any(unix, windows))]
impl From<PathBuf> for PortableOsString {
    fn from(s: PathBuf) -> Self {
        OsString::from(s).into()
    }
}

impl From<String> for PortableOsString {
    fn from(s: String) -> Self {
        Self::Undifferentiated(s)
    }
}

impl From<&str> for PortableOsString {
    fn from(s: &str) -> Self {
        String::from(s).into()
    }
}

impl TryFrom<PortableOsString> for OsString {
    type Error = (&'static str, PortableOsString);
    #[cfg(not(any(unix, windows)))]
    fn try_from(s: PortableOsString) -> Result<Self, Self::Error> {
        match s {
            PortableOsString::Undifferentiated(s) => Ok(s.into()),
            _ => Err(("Concrete OsStrings not convertible", s)),
        }
    }
    #[cfg(unix)]
    fn try_from(s: PortableOsString) -> Result<Self, Self::Error> {
        match s {
            PortableOsString::Unix { data, .. } => {
                use std::os::unix::ffi::OsStringExt;
                Ok(Self::from_vec(data))
            }
            PortableOsString::Undifferentiated(s) => Ok(s.into()),
            _ => Err(("Windows OsStrings not convertible to Unix", s)),
        }
    }
    #[cfg(windows)]
    fn try_from(s: PortableOsString) -> Result<Self, Self::Error> {
        match s {
            PortableOsString::Windows { data, .. } => {
                use std::os::windows::ffi::OsStringExt;
                Ok(Self::from_wide(data))
            }
            PortableOsString::Undifferentiated(s) => Ok(s.into()),
            _ => Err(("Windows OsStrings not convertible to Unix", s)),
        }
    }
}

impl TryFrom<PortableOsString> for PathBuf {
    type Error = (&'static str, PortableOsString);
    fn try_from(s: PortableOsString) -> Result<Self, Self::Error> {
        let s = OsString::try_from(s)?;
        Ok(PathBuf::from(s))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum FileEntry {
    GCSFile(PortableOsString),
    Directory(PortableOsString),
}

pub mod as_cbor {
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
pub struct ReadResponse {
    pub path: PortableOsString,
    #[serde(with = "as_cbor")]
    pub contents: gcs::FileKind,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WriteRequest {
    pub path: PortableOsString,
    #[serde(with = "as_cbor")]
    pub contents: gcs::FileKind,
}
