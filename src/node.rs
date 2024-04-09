use std::path::Path;
use std::sync::{Arc, Weak, RwLock};
use std::collections::HashMap;
use crate::property::WzValue;
use crate::{ WzObjectType, WzDirectoryParseError, WzFileParseError, WzImageParseError, WzFile};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NodeParseError {
    #[error("Node has been using")]
    NodeHasBeenUsing,

    #[error("Error parsing WzDirectory: {0}")]
    WzDirectoryParseError(#[from] WzDirectoryParseError),

    #[error("Error parsing WzFile: {0}")]
    WzFileParseError(#[from] WzFileParseError),

    #[error("Error parsing WzImage: {0}")]
    WzImageParseError(#[from] WzImageParseError),

    #[error("Node not found")]
    NodeNotFound,
}

#[derive(Debug)]
pub struct WzNode {
    pub name: String,
    pub object_type: WzObjectType,
    pub parent: Weak<RwLock<WzNode>>,
    pub children: HashMap<String, Arc<RwLock<WzNode>>>,
}

pub type WzNodeArc = Arc<RwLock<WzNode>>;
pub type WzNodeArcVec = Vec<(String, WzNodeArc)>;

impl From<WzNode> for WzNodeArc {
    fn from(node: WzNode) -> Self {
        node.into_lock()
    }
}

impl WzNode {
    pub fn new(name: String, object_type: WzObjectType, parent: Option<&WzNodeArc>) -> Self {
        Self {
            name,
            object_type,
            parent: parent.map(Arc::downgrade).unwrap_or(Weak::new()),
            children: HashMap::new(),
        }
    }
    pub fn from_wz_file(path: &str, parent: Option<&WzNodeArc>) -> Result<Self, NodeParseError> {
        let name = Path::new(path).file_stem().unwrap().to_str().unwrap().to_string();
        let wz_file = WzFile::from_file(path)?;
        return Ok(WzNode::new(
            name, 
            WzObjectType::File(Box::new(wz_file)), 
            parent
        ));
    }
    pub fn into_lock(self) -> WzNodeArc {
        Arc::new(RwLock::new(self))
    }
    pub fn parse(&mut self, parent: &WzNodeArc) -> Result<(), NodeParseError> {
        let childs: WzNodeArcVec;

        match self.object_type {
            WzObjectType::Directory(ref mut directory) => {
                if directory.is_parsed {
                    return Ok(());
                }
                childs = directory.resolve_children(parent)?;
            },
            WzObjectType::File(ref mut file) => {
                if file.is_parsed {
                    return Ok(());
                }
                childs = file.parse(parent, None)?;
            },
            WzObjectType::Image(ref mut image) => {
                if image.is_parsed {
                    return Ok(());
                }
                childs = image.resolve_children(parent)?;
            },
            _ => return Ok(()),
        }
        
        for (name, child) in childs {
            self.children.insert(name, child);
        }

        Ok(())
    }

    pub fn get_full_path(&self) -> String {
        let mut path = self.name.clone();
        let mut parent = self.parent.upgrade();
        loop {
            if let Some(parent_inner) = parent {
                let read = parent_inner.read().unwrap();
                path = format!("{}/{}", read.name, path);
                parent = read.parent.upgrade();
            } else {
                break;
            }
        }
        path
    }

    pub fn at(&self, name: &str) -> Option<WzNodeArc> {
        self.children.get(name).cloned()
    }
    pub fn at_relative(&self, path: &str) -> Option<WzNodeArc> {
        if path == ".." {
            self.parent.upgrade()
        } else {
            self.at(path)
        }
    }
    pub fn at_path(&self, path: &str) -> Option<WzNodeArc> {
        let mut pathes = path.split("/");
        let first = self.at(pathes.next().unwrap());
        if let Some(first) = first {
            pathes.try_fold(first, |node, name| node.read().unwrap().at(name))
        } else {
            None
        }
    }
    pub fn at_path_parsed(&mut self, path: &str) -> Result<WzNodeArc, NodeParseError> {
        let mut pathes = path.split("/");
        
        let first = self.at(pathes.next().unwrap());
        if let Some(first) = first {
            pathes.try_fold(first, |node, name| {
                let mut write = node.write().unwrap();
                write.parse(&node)?;
                write.at(name).ok_or(NodeParseError::NodeNotFound)
            })
        } else {
            Err(NodeParseError::NodeNotFound)
        }
    }
    pub fn at_path_relative(&self, path: &str) -> Option<WzNodeArc> {
        let mut pathes = path.split("/");
        let first = self.at_relative(pathes.next().unwrap());
        if let Some(first) = first {
            pathes.try_fold(first, |node, name| node.read().unwrap().at_relative(name))
        } else {
            None
        }
    }
    pub fn filter_parent<F>(&self, cb: F) -> Option<WzNodeArc> 
        where F: Fn(&WzNode) -> bool
    {
        let mut parent = self.parent.upgrade();
        loop {
            if let Some(parent_inner) = parent {
                let read = parent_inner.read().unwrap();
                if cb(&read) {
                    break Some(Arc::clone(&parent_inner))
                } else {
                    parent = read.parent.upgrade();
                }
            } else {
                break None;
            }
        }
    }
    pub fn get_parent_wz_image(&self) -> Option<WzNodeArc> {
        self.filter_parent(|node| matches!(node.object_type, WzObjectType::Image(_)))
    }
    pub fn get_base_wz_file(&self) -> Option<WzNodeArc> {
        self.filter_parent(|node| matches!(node.object_type, WzObjectType::File(_)) && node.name.as_str() == "Base")
    }

    pub fn resolve_inlink(&self, node: &WzNodeArc) -> Option<WzNodeArc> {
        let path = match &node.read().unwrap().object_type {
            WzObjectType::Value(WzValue::String(meta)) => {
                if let Ok(path) = meta.get_string() {
                    path
                } else {
                    return None;
                }
            },
            _ => {
                println!("node _inlink not a WzString");
                return None
            },
        };

        let parent_wz_image = self.get_parent_wz_image()?;
        let parent_wz_image = parent_wz_image.read().unwrap();
        parent_wz_image.at_path(&path)
    }

    pub fn resolve_outlink(&self, node: &WzNodeArc, force_parse: bool) -> Option<WzNodeArc> {
        let path = match &node.read().unwrap().object_type {
            WzObjectType::Value(WzValue::String(meta)) => {
                if let Ok(path) = meta.get_string() {
                    path
                } else {
                    return None;
                }
            },
            _ => {
                println!("node _outlink not a WzString");
                return None
            },
        };

        let parent_wz_base = self.get_base_wz_file()?;

        if force_parse {
            parent_wz_base.write().unwrap().at_path_parsed(&path).ok()
        } else {
            parent_wz_base.read().unwrap().at_path(&path)
        }
    }

    pub fn transfer_childs(&mut self, to: &WzNodeArc) {
        let mut write = to.write().unwrap();
        for (name, child) in self.children.drain() {
            write.children.insert(name, child);
        }
    }
}