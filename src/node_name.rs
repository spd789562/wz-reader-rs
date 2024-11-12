use hashbrown::Equivalent;
use std::fmt::Display;
use std::ops::Deref;
use std::sync::Arc;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A wrapper around `Arc<str>` use for WzNode's name and hashmap key.
#[cfg_attr(feature = "serde", derive(Deserialize))]
#[cfg_attr(feature = "serde", serde(from = "String"))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WzNodeName(Arc<str>);

impl Equivalent<WzNodeName> for str {
    #[inline]
    fn equivalent(&self, key: &WzNodeName) -> bool {
        self == key.as_str()
    }
}

impl From<&str> for WzNodeName {
    #[inline]
    fn from(s: &str) -> Self {
        WzNodeName(Arc::from(s))
    }
}

impl From<String> for WzNodeName {
    #[inline]
    fn from(s: String) -> Self {
        WzNodeName(Arc::from(s))
    }
}

impl Deref for WzNodeName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for WzNodeName {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for WzNodeName {
    #[inline]
    fn default() -> Self {
        "".into()
    }
}

impl WzNodeName {
    #[inline]
    pub fn new(s: &str) -> Self {
        s.into()
    }
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(feature = "serde")]
impl Serialize for WzNodeName {
    /// I don't known how to directly into &str, so impl this
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}
