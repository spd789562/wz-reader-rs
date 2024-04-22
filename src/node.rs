use std::path::Path;
use std::sync::{Arc, Weak, RwLock};
use hashbrown::HashMap;
use crate::{ version, WzDirectoryParseError, WzFile, WzFileParseError, WzImage, WzImageParseError, WzObjectType, WzNodeName};
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
    pub name: WzNodeName,
    pub object_type: WzObjectType,
    pub parent: Weak<RwLock<WzNode>>,
    pub children: HashMap<WzNodeName, Arc<RwLock<WzNode>>>,
}

pub type WzNodeArc = Arc<RwLock<WzNode>>;
pub type WzNodeArcVec = Vec<(WzNodeName, WzNodeArc)>;

impl From<WzNode> for WzNodeArc {
    fn from(node: WzNode) -> Self {
        node.into_lock()
    }
}

impl WzNode {
    pub fn new(name: &WzNodeName, object_type: WzObjectType, parent: Option<&WzNodeArc>) -> Self {
        Self {
            name: name.clone(),
            object_type,
            parent: parent.map(Arc::downgrade).unwrap_or_default(),
            children: HashMap::new(),
        }
    }
    pub fn from_str(name: &str, object_type: WzObjectType, parent: Option<&WzNodeArc>) -> Self {
        Self::new(&name.into(), object_type, parent)
    }
    pub fn from_wz_file(path: &str, version: Option<version::WzMapleVersion>, patch_version: Option<i32>, parent: Option<&WzNodeArc>) -> Result<Self, NodeParseError> {
        let name = Path::new(path).file_stem().unwrap().to_str().unwrap();
        let version = version.unwrap_or(version::WzMapleVersion::EMS);
        let wz_file = WzFile::from_file(path, version::get_iv_by_maple_version(version), patch_version)?;
        Ok(WzNode::new(
            &name.into(), 
            WzObjectType::File(Box::new(wz_file)), 
            parent
        ))
    }
    pub fn from_img_file(path: &str, version: Option<version::WzMapleVersion>, parent: Option<&WzNodeArc>) -> Result<Self, NodeParseError> {
        let name = Path::new(path).file_stem().unwrap().to_str().unwrap();
        let version = version.unwrap_or(version::WzMapleVersion::EMS);
        let wz_image = WzImage::from_file(path, version::get_iv_by_maple_version(version))?;
        Ok(WzNode::new(
            &name.into(), 
            WzObjectType::Image(Box::new(wz_image)), 
            parent
        ))
    }

    pub fn into_lock(self) -> WzNodeArc {
        Arc::new(RwLock::new(self))
    }

    pub fn parse(&mut self, parent: &WzNodeArc) -> Result<(), NodeParseError> {
        let childs: WzNodeArcVec = match self.object_type {
            WzObjectType::Directory(ref mut directory) => {
                if directory.is_parsed {
                    return Ok(());
                }
                directory.resolve_children(parent)?
            },
            WzObjectType::File(ref mut file) => {
                if file.is_parsed {
                    return Ok(());
                }
                file.parse(parent, None)?
            },
            WzObjectType::Image(ref mut image) => {
                if image.is_parsed {
                    return Ok(());
                }
                image.resolve_children(parent)?
            },
            _ => return Ok(()),
        };
        
        for (name, child) in childs {
            self.children.insert(name, child);
        }

        Ok(())
    }
    pub fn unparse(&mut self) -> Result<(), NodeParseError> {
        match &mut self.object_type {
            WzObjectType::Directory(directory) => {
                directory.is_parsed = false;
            },
            WzObjectType::File(file) => {
                file.is_parsed = false;
            },
            WzObjectType::Image(image) => {
                image.is_parsed = false;
            },
            _ => return Ok(()),
        }
        
        self.children.clear();

        Ok(())
    }

    pub fn get_full_path(&self) -> String {
        let mut path = self.name.to_string();
        let mut parent = self.parent.upgrade();
        while let Some(parent_inner) = parent {
            let read = parent_inner.read().unwrap();
            path = format!("{}/{}", &read.name, path);
            parent = read.parent.upgrade();
        }
        path
    }

    pub fn at(&self, name: &str) -> Option<WzNodeArc> {
        self.children.get(name).map(Arc::clone)
    }
    pub fn at_relative(&self, path: &str) -> Option<WzNodeArc> {
        if path == ".." {
            self.parent.upgrade()
        } else {
            self.at(path)
        }
    }
    pub fn at_path(&self, path: &str) -> Option<WzNodeArc> {
        let mut pathes = path.split('/');
        let first = self.at(pathes.next().unwrap());
        if let Some(first) = first {
            pathes.try_fold(first, |node, name| node.read().unwrap().at(name))
        } else {
            None
        }
    }
    pub fn at_path_parsed(&self, path: &str) -> Result<WzNodeArc, NodeParseError> {
        let mut pathes = path.split('/');
        
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
        let mut pathes = path.split('/');
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

    pub fn transfer_childs(&mut self, to: &WzNodeArc) {
        let mut write = to.write().unwrap();
        for (name, child) in self.children.drain() {
            write.children.insert(name, child);
        }
    }
}

pub fn resolve_inlink(path: &str, node: &WzNodeArc) -> Option<WzNodeArc> {
    let parent_wz_image = node.read().unwrap().get_parent_wz_image()?;
    let parent_wz_image = parent_wz_image.read().unwrap();
    parent_wz_image.at_path(path)
}

pub fn resolve_outlink(path: &str, node: &WzNodeArc, force_parse: bool) -> Option<WzNodeArc> {
    let parent_wz_base = node.read().unwrap().get_base_wz_file()?;

    if force_parse {
        parent_wz_base.write().unwrap().at_path_parsed(path).ok()
    } else {
        parent_wz_base.read().unwrap().at_path(path)
    }
}