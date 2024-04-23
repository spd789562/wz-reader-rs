use std::sync::Arc;
use std::ops::Deref;
use std::fmt::Display;
use hashbrown::Equivalent;

/// A wrapper around `Arc<str>` use for WzNode's name and hashmap key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WzNodeName(Arc<str>);

impl Equivalent<WzNodeName> for str {
    fn equivalent(&self, key: &WzNodeName) -> bool {
        self == key.as_str()
    }
}

impl From<&str> for WzNodeName {
    fn from(s: &str) -> Self {
        WzNodeName(Arc::from(s))
    }
}

impl From<String> for WzNodeName {
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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl WzNodeName {
    pub fn new(s: &str) -> Self {
        s.into()
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}