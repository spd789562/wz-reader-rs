use std::path::Path;
use std::sync::{Arc, Weak, RwLock};
use hashbrown::HashMap;
use crate::{ version, directory, wz_image, file, WzFile, WzImage, WzObjectType, WzNodeName};

#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Node has been using")]
    NodeHasBeenUsing,

    #[error("Error parsing WzDirectory: {0}")]
    WzDirectoryParseError(#[from] directory::Error),

    #[error("Error parsing WzFile: {0}")]
    WzFileParseError(#[from] file::Error),

    #[error("Error parsing WzImage: {0}")]
    WzImageParseError(#[from] wz_image::Error),

    #[error("Node not found")]
    NodeNotFound,
}

/// A basic unit of wz_reader
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug)]
pub struct WzNode {
    pub name: WzNodeName,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub object_type: WzObjectType,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub parent: Weak<RwLock<WzNode>>,
    #[cfg_attr(feature = "serde", serde(with = "arc_node_serde"))]
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
    pub fn new(name: &WzNodeName, object_type: impl Into<WzObjectType>, parent: Option<&WzNodeArc>) -> Self {
        Self {
            name: name.clone(),
            object_type: object_type.into(),
            parent: parent.map(Arc::downgrade).unwrap_or_default(),
            children: HashMap::new(),
        }
    }
    /// Create a `WzNode` use &str as name.
    pub fn from_str(name: &str, object_type: impl Into<WzObjectType>, parent: Option<&WzNodeArc>) -> Self {
        Self::new(&name.into(), object_type, parent)
    }
    /// Create a `WzNode` from a any `.wz` file.
    pub fn from_wz_file(path: &str, version: Option<version::WzMapleVersion>, patch_version: Option<i32>, parent: Option<&WzNodeArc>) -> Result<Self, Error> {
        let name = Path::new(path).file_stem().unwrap().to_str().unwrap();
        let version = version.unwrap_or(version::WzMapleVersion::BMS);
        let wz_file = WzFile::from_file(path, version::get_iv_by_maple_version(version), patch_version)?;
        Ok(WzNode::new(
            &name.into(), 
            wz_file, 
            parent
        ))
    }
    /// Create a `WzNode` from a any `.img` file. If version is not provided, it will try to detect the version.
    pub fn from_img_file(path: &str, version: Option<version::WzMapleVersion>, parent: Option<&WzNodeArc>) -> Result<Self, Error> {
        let wz_image = WzImage::from_file(path, version.map(version::get_iv_by_maple_version))?;
        Ok(WzNode::new(
            &wz_image.name.clone(), 
            wz_image, 
            parent
        ))
    }

    /// Create a `WzNode` from a any `.img` file with custom wz iv([u8; 4])
    pub fn from_img_file_with_iv(path: &str, iv: [u8; 4], parent: Option<&WzNodeArc>) -> Result<Self, Error> {
        let wz_image = WzImage::from_file(path, Some(iv))?;
        Ok(WzNode::new(
            &wz_image.name.clone(), 
            wz_image, 
            parent
        ))
    }

    /// A quicker way to turn `WzNode` to `WzNodeArc`.
    pub fn into_lock(self) -> WzNodeArc {
        Arc::new(RwLock::new(self))
    }

    /// Parse the node base on the object type.
    pub fn parse(&mut self, parent: &WzNodeArc) -> Result<(), Error> {
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

    /// Clear the node childrens and set the node to unparsed.
    pub fn unparse(&mut self) -> Result<(), Error> {
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

    pub fn add(&mut self, node: &WzNodeArc) {
        self.children.insert(node.read().unwrap().name.clone(), Arc::clone(node));
    }

    /// Returns the full path of the WzNode.
    ///
    /// # Examples
    ///
    /// ```
    /// # use wz_reader::{WzObjectType, WzNode};
    /// # use wz_reader::property::WzValue;
    /// let root = WzNode::from_str("root", 1, None).into_lock();
    /// let child = WzNode::from_str("1", 1, Some(&root)).into_lock();
    /// let grandchild = WzNode::from_str("2", 1, Some(&child)).into_lock();
    /// 
    /// assert_eq!(grandchild.read().unwrap().get_full_path(), "root/1/2");
    /// ```
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
    /// A alias to get child.
    ///
    /// # Examples
    ///
    /// ```
    /// # use wz_reader::{WzObjectType, WzNode};
    /// # use wz_reader::property::WzValue;
    /// let root = WzNode::from_str("root", 1, None).into_lock();
    /// let child1 = WzNode::from_str("1", 1, Some(&root)).into_lock();
    /// let child2 = WzNode::from_str("2", 1, Some(&root)).into_lock();
    /// 
    /// let mut root =  root.write().unwrap();
    /// root.add(&child1);
    /// root.add(&child2);
    /// 
    /// assert!(root.at("1").is_some());
    /// assert!(root.at("3").is_none());
    /// ```
    pub fn at(&self, name: &str) -> Option<WzNodeArc> {
        self.children.get(name).map(Arc::clone)
    }

    /// A relative path version of `at` able to use `..` to get parent node.
    ///
    /// # Examples
    ///
    /// ```
    /// # use wz_reader::{WzObjectType, WzNode};
    /// # use wz_reader::property::WzValue;
    /// # use std::sync::Arc;
    /// let root = WzNode::from_str("root", 1, None).into_lock();
    /// let child1 = WzNode::from_str("1", 1, Some(&root)).into_lock();
    /// let child2 = WzNode::from_str("2", 1, Some(&root)).into_lock();
    /// 
    /// let mut root =  root.write().unwrap();
    /// root.add(&child1);
    /// root.add(&child2);
    /// 
    /// assert!(child1.read().unwrap().at_relative("..").is_some());
    /// assert!(root.at_relative("..").is_none());
    /// ```
    pub fn at_relative(&self, path: &str) -> Option<WzNodeArc> {
        if path == ".." {
            self.parent.upgrade()
        } else {
            self.at(path)
        }
    }
    /// Get node by path like `a/b/c`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use wz_reader::{WzObjectType, WzNode};
    /// # use wz_reader::property::WzValue;
    /// let root = WzNode::from_str("root", 1, None).into_lock();
    /// let child1 = WzNode::from_str("1", 1, Some(&root)).into_lock();
    /// let child2 = WzNode::from_str("2", 1, Some(&child1)).into_lock();
    /// 
    /// root.write().unwrap().add(&child1);
    /// child1.write().unwrap().add(&child2);
    /// 
    /// assert!(root.read().unwrap().at_path("1/2").is_some());
    /// assert!(root.read().unwrap().at_path("1/3").is_none());
    /// ```
    pub fn at_path(&self, path: &str) -> Option<WzNodeArc> {
        let mut pathes = path.split('/');
        let first = self.at(pathes.next().unwrap());
        if let Some(first) = first {
            pathes.try_fold(first, |node, name| node.read().unwrap().at(name))
        } else {
            None
        }
    }
    /// Get node by path like `a/b/c` and parse all nodes in the path.
    pub fn at_path_parsed(&self, path: &str) -> Result<WzNodeArc, Error> {
        let mut pathes = path.split('/');
        
        let first = self.at(pathes.next().unwrap());
        if let Some(first) = first {
            pathes.try_fold(first, |node, name| {
                let mut write = node.write().unwrap();
                write.parse(&node)?;
                write.at(name).ok_or(Error::NodeNotFound)
            })
        } else {
            Err(Error::NodeNotFound)
        }
    }
    /// Get node by path that include relative path like `../../b/c`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use wz_reader::{WzObjectType, WzNode};
    /// # use wz_reader::property::WzValue;
    /// let root = WzNode::from_str("root", 1, None).into_lock();
    /// let child1 = WzNode::from_str("1", 1, Some(&root)).into_lock();
    /// let child2 = WzNode::from_str("2", 1, Some(&child1)).into_lock();
    /// 
    /// root.write().unwrap().add(&child1);
    /// child1.write().unwrap().add(&child2);
    /// 
    /// assert!(child2.read().unwrap().at_path_relative("../..").is_some());
    /// assert!(child2.read().unwrap().at_path_relative("../3").is_none());
    /// ```
    pub fn at_path_relative(&self, path: &str) -> Option<WzNodeArc> {
        let mut pathes = path.split('/');
        let first = self.at_relative(pathes.next().unwrap());
        if let Some(first) = first {
            pathes.try_fold(first, |node, name| node.read().unwrap().at_relative(name))
        } else {
            None
        }
    }
    
    /// Get parent node by filter.
    /// 
    /// # Examples
    /// 
    /// ```
    /// # use wz_reader::{WzObjectType, WzNode};
    /// # use wz_reader::property::WzValue;
    /// let root = WzNode::from_str("root", 1, None).into_lock();
    /// let child1 = WzNode::from_str("1", 1, Some(&root)).into_lock();
    /// let child2 = WzNode::from_str("2", 1, Some(&child1)).into_lock();
    /// 
    /// root.write().unwrap().add(&child1);
    /// child1.write().unwrap().add(&child2);
    /// 
    /// let target = child2.read().unwrap().filter_parent(|node| node.name.as_str() == "root");
    /// assert!(target.is_some());
    /// ```
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

    /// Transfer all children to another node. It will merge the children instead of replace to new one.
    pub fn transfer_childs(&mut self, to: &WzNodeArc) {
        let mut write = to.write().unwrap();
        for (name, child) in self.children.drain() {
            if let Some(old) = write.children.get(&name) {
                child.write().unwrap().transfer_childs(old);
            } else {
                child.write().unwrap().parent = Arc::downgrade(&to);
                write.children.insert(name, child);
            }
        }
    }

    /// Generate full json that can deserialize back to `WzNode`.
    #[cfg(feature = "json")]
    pub fn to_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }

    /// Generate simple json only name and value.
    #[cfg(feature = "json")]
    pub fn to_simple_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        use serde_json::{Map, Value, to_value};
        use crate::property::WzSubProperty;


        if self.children.is_empty() {
            match &self.object_type {
                WzObjectType::Value(value_type) => {
                    return Ok(value_type.clone().into())
                },
                WzObjectType::Property(WzSubProperty::PNG(inner)) => {
                    return to_value(inner)
                }
                WzObjectType::Property(WzSubProperty::Sound(inner)) => {
                    return to_value(inner)
                },
                _ => {
                    return Ok(Value::Null)
                }
            }
        }

        let mut json = Map::new();

        match &self.object_type {
            WzObjectType::Property(WzSubProperty::PNG(inner)) => {
                let dict =  to_value(inner)?;

                if let Value::Object(dict) = dict {
                    for (name, value) in dict {
                        json.insert(name, value);
                    }
                }
            },
            WzObjectType::Property(WzSubProperty::Sound(inner)) => {
                let dict =  to_value(inner)?;

                if let Value::Object(dict) = dict {
                    for (name, value) in dict {
                        json.insert(name, value);
                    }
                }
            },
            _ => {}
        }

        for (name, value) in self.children.iter() {
            let child = value.read().unwrap();
            json.insert(name.to_string(), child.to_simple_json()?);
        }

        Ok(Value::Object(json))
    }
}

/// Just wrap around of `node.write().unwrap().parse(&node)`
pub fn parse_node(node: &WzNodeArc) -> Result<(), Error> {
    node.write().unwrap().parse(node)
}

/// Resolve a `_inlink` path, a `_inlink` path always start from a `WzImage`.
pub fn resolve_inlink(path: &str, node: &WzNodeArc) -> Option<WzNodeArc> {
    let parent_wz_image = node.read().unwrap().get_parent_wz_image()?;
    let parent_wz_image = parent_wz_image.read().unwrap();
    parent_wz_image.at_path(path)
}

/// Resolve a `_outlink` path, a `_outlink` path always start from Wz's data root(a.k.a `Base.wz`).
pub fn resolve_outlink(path: &str, node: &WzNodeArc, force_parse: bool) -> Option<WzNodeArc> {
    let parent_wz_base = node.read().unwrap().get_base_wz_file()?;

    if force_parse {
        parent_wz_base.write().unwrap().at_path_parsed(path).ok()
    } else {
        parent_wz_base.read().unwrap().at_path(path)
    }
}

/// Make sure WzNode tree's all node has correct parent.
pub fn resolve_childs_parent(node: &WzNodeArc) {
    let node_read = node.read().unwrap();
    for child in node_read.children.values() {
        child.write().unwrap().parent = Arc::downgrade(&node);
        resolve_childs_parent(&child);
    }
}

#[cfg(feature = "serde")]
mod arc_node_serde {
    use serde::{Deserialize, Serialize};
    use serde::ser::{SerializeMap, Serializer};
    use serde::de::Deserializer;
    use std::sync::{Arc, RwLock};
    use hashbrown::HashMap;
    use crate::WzNodeName;

    pub fn serialize<S, T>(val: &HashMap<WzNodeName, Arc<RwLock<T>>>, s: S) -> Result<S::Ok, S::Error>
        where S: Serializer,
              T: Serialize,
    {
        let mut map = s.serialize_map(Some(val.len()))?;
        for (k, v) in val {
            map.serialize_entry(k, &*v.read().unwrap())?;
        }
        map.end()
    }

    pub fn deserialize<'de, D, T>(d: D) -> Result<HashMap<WzNodeName, Arc<RwLock<T>>>, D::Error>
        where D: Deserializer<'de>,
              T: Deserialize<'de>,
    {
        let map = HashMap::<WzNodeName, T>::deserialize(d)?;
        Ok(map.into_iter().map(|(k, v)| (k, Arc::new(RwLock::new(v)))).collect())
    }
}

#[cfg(feature = "serde")]
#[cfg(test)]
mod test {
    use super::*;
    
    #[cfg(feature = "serde")]
    use serde_json::json;

    #[test]
    fn test_serialize_wz_node() {
        let root = WzNode::from_str("root", 1, None).into_lock();
        let child1 = WzNode::from_str("1", 1, Some(&root)).into_lock();
        let child2  = WzNode::from_str("2", 1, Some(&root)).into_lock();
        root.write().unwrap().add(&child1);
        root.write().unwrap().add(&child2);
        let child11 = WzNode::from_str("1-1", 1, Some(&child1)).into_lock();
        let child12 = WzNode::from_str("1-2", 1, Some(&child1)).into_lock();
        child1.write().unwrap().add(&child11);
        child1.write().unwrap().add(&child12);
        let child111 = WzNode::from_str("1-1-1", 1, Some(&child11)).into_lock();
        child11.write().unwrap().add(&child111);
        
        let json = serde_json::to_value(&*root.read().unwrap()).unwrap();

        let result = json!({
            "name": "root",
            "type": "Int",
            "data": 1,
            "children": {
                "1": {
                    "name": "1",
                    "type": "Int",
                    "data": 1,
                    "children": {
                        "1-1": {
                            "name": "1-1",
                            "type": "Int",
                            "data": 1,
                            "children": {
                                "1-1-1": {
                                    "name": "1-1-1",
                                    "type": "Int",
                                    "data": 1,
                                    "children": {}
                                }
                            }
                        },
                        "1-2": {
                            "name": "1-2",
                            "type": "Int",
                            "data": 1,
                            "children": {}
                        }
                    }
                },
                "2": {
                    "name": "2",
                    "type": "Int",
                    "data": 1,
                    "children": {}
                }
            }
        });

        assert_eq!(json, result);
    }

    #[test]
    fn test_deserialize_wz_node() {
        let source = json!({
            "name": "root",
            "type": "Int",
            "data": 1,
            "children": {
                "1": {
                    "name": "1",
                    "type": "Int",
                    "data": 1,
                    "children": {
                        "1-1": {
                            "name": "1-1",
                            "type": "Int",
                            "data": 1,
                            "children": {
                                "1-1-1": {
                                    "name": "1-1-1",
                                    "type": "Int",
                                    "data": 1,
                                    "children": {}
                                }
                            }
                        },
                        "1-2": {
                            "name": "1-2",
                            "type": "Int",
                            "data": 1,
                            "children": {}
                        }
                    }
                },
                "2": {
                    "name": "2",
                    "type": "Int",
                    "data": 1,
                    "children": {}
                }
            }
        });

        let root = serde_json::from_value::<WzNode>(source).unwrap().into_lock();

        let node111 = root.read().unwrap().at_path("1/1-1/1-1-1");

        assert!(node111.is_some());

        let node111 = node111.unwrap();

        assert_eq!(node111.read().unwrap().name.as_str(), "1-1-1");
        // should not be able to resolve parent
        assert!(node111.read().unwrap().parent.upgrade().is_none());

        resolve_childs_parent(&root);

        // should able to get parent after resolved
        let node111_parent = node111.read().unwrap().parent.upgrade();
        assert!(node111_parent.is_some());

        let node111_parent = node111_parent.unwrap();

        assert_eq!(node111_parent.read().unwrap().name.as_str(), "1-1");
    }

    #[cfg(feature = "json")]
    #[test]
    fn test_node_to_simple_json() {
        use crate::property::{WzPng, WzSound};

        let root = WzNode::from_str("root", 1, None).into_lock();
        let child1 = WzNode::from_str("1", 1, Some(&root)).into_lock();
        let child2  = WzNode::from_str("2", 1, Some(&root)).into_lock();
        root.write().unwrap().add(&child1);
        root.write().unwrap().add(&child2);
        let child11 = WzNode::from_str("1-1", 1, Some(&child1)).into_lock();
        let child12 = WzNode::from_str("1-2", 1, Some(&child1)).into_lock();
        child1.write().unwrap().add(&child11);
        child1.write().unwrap().add(&child12);
        let child111 = WzNode::from_str("1-1-1", 1, Some(&child11)).into_lock();
        let child112 = WzNode::from_str("1-1-2", 2, Some(&child11)).into_lock();
        let child11png = WzNode::from_str("1-1-png", WzPng::default(), Some(&child11)).into_lock();
        let child11sound = WzNode::from_str("1-1-sound", WzSound::default(), Some(&child11)).into_lock();
        child11.write().unwrap().add(&child111);
        child11.write().unwrap().add(&child112);
        child11.write().unwrap().add(&child11png);
        child11.write().unwrap().add(&child11sound);
        let child11pngz = WzNode::from_str("1-1-png-z", 2, Some(&child11png)).into_lock();
        child11png.write().unwrap().add(&child11pngz);

        let json = root.read().unwrap().to_simple_json().unwrap();

        let result = json!({
            "1": {
                "1-1": {
                    "1-1-1": 1,
                    "1-1-2": 2,
                    "1-1-png": {
                        "width": 0,
                        "height": 0,
                        "1-1-png-z": 2
                    },
                    "1-1-sound": {
                        "duration": 0,
                        "sound_type": "Binary"
                    }
                },
                "1-2": 1
            },
            "2": 1
        });

        assert_eq!(json, result);
    }
}