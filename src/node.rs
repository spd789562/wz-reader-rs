use std::path::Path;
use std::sync::{Arc, Weak, RwLock};
use hashbrown::HashMap;
use crate::property::{WzValue, WzSubProperty, WzSound, WzPng, WzString, WzLua, WzRawData, Vector2D};
use crate::{ version, WzDirectory, WzDirectoryParseError, WzFile, WzFileParseError, WzImage, WzImageParseError, WzObjectType};
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
            parent: parent.map(Arc::downgrade).unwrap_or_default(),
            children: HashMap::new(),
        }
    }
    pub fn from_wz_file(path: &str, version: Option<version::WzMapleVersion>, patch_version: Option<i32>, parent: Option<&WzNodeArc>) -> Result<Self, NodeParseError> {
        let name = Path::new(path).file_stem().unwrap().to_str().unwrap().to_string();
        let version = version.unwrap_or(version::WzMapleVersion::EMS);
        let wz_file = WzFile::from_file(path, version::get_iv_by_maple_version(version), patch_version)?;
        Ok(WzNode::new(
            name, 
            WzObjectType::File(Box::new(wz_file)), 
            parent
        ))
    }
    pub fn from_img_file(path: &str, version: Option<version::WzMapleVersion>, parent: Option<&WzNodeArc>) -> Result<Self, NodeParseError> {
        let name = Path::new(path).file_stem().unwrap().to_str().unwrap().to_string();
        let version = version.unwrap_or(version::WzMapleVersion::EMS);
        let wz_image = WzImage::from_file(path, version::get_iv_by_maple_version(version))?;
        Ok(WzNode::new(
            name, 
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
        let mut path = self.name.clone();
        let mut parent = self.parent.upgrade();
        while let Some(parent_inner) = parent {
            let read = parent_inner.read().unwrap();
            path = format!("{}/{}", read.name, path);
            parent = read.parent.upgrade();
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
    parent_wz_image.at_path(&path)
}

pub fn resolve_outlink(path: &str, node: &WzNodeArc, force_parse: bool) -> Option<WzNodeArc> {
    let parent_wz_base = node.read().unwrap().get_base_wz_file()?;

    if force_parse {
        parent_wz_base.write().unwrap().at_path_parsed(&path).ok()
    } else {
        parent_wz_base.read().unwrap().at_path(&path)
    }
}

pub trait WzNodeCast {
    fn try_as_file(&self) -> Option<&Box<WzFile>>;
    fn try_as_directory(&self) -> Option<&Box<WzDirectory>>;
    fn try_as_image(&self) -> Option<&Box<WzImage>>;

    fn try_as_sub_property(&self) -> Option<&WzSubProperty>;
    fn try_as_value(&self) -> Option<&WzValue>;

    fn try_as_png(&self) -> Option<&Box<WzPng>>;
    fn try_as_sound(&self) -> Option<&Box<WzSound>>;
    fn try_as_string(&self) -> Option<&WzString>;
    fn try_as_lua(&self) -> Option<&WzLua>;
    fn try_as_raw_data(&self) -> Option<&WzRawData>;

    fn try_as_vector2d(&self) -> Option<&Vector2D>;
    fn try_as_short(&self) -> Option<&i16>;
    fn try_as_int(&self) -> Option<&i32>;
    fn try_as_long(&self) -> Option<&i64>;
    fn try_as_float(&self) -> Option<&f32>;
    fn try_as_double(&self) -> Option<&f64>;
}

macro_rules! try_as {
    ($func_name:ident, $variant:ident, $result:ty) => {
        fn $func_name(&self) -> Option<&$result> {
            match &self.object_type {
                WzObjectType::$variant(inner) => Some(inner),
                _ => None,
            }
        }
    };
}

macro_rules! try_as_wz_value {
    ($func_name:ident, $variant:ident, $result:ident) => {
        fn $func_name(&self) -> Option<&$result> {
            match &self.object_type {
                WzObjectType::Value(WzValue::$variant(inner)) => Some(inner),
                _ => None,
            }
        }
    };
}

impl WzNodeCast for WzNode {
    try_as!(try_as_file, File, Box<WzFile>);
    try_as!(try_as_directory, Directory, Box<WzDirectory>);
    try_as!(try_as_image, Image, Box<WzImage>);

    try_as!(try_as_sub_property, Property, WzSubProperty);
    try_as!(try_as_value, Value, WzValue);

    fn try_as_png(&self) -> Option<&Box<WzPng>> {
        match &self.object_type {
            WzObjectType::Property(WzSubProperty::PNG(png)) => Some(png),
            _ => None,
        }
    }
    fn try_as_sound(&self) -> Option<&Box<WzSound>> {
        match &self.object_type {
            WzObjectType::Property(WzSubProperty::Sound(sound)) => Some(sound),
            _ => None,
        }
    }
    fn try_as_string(&self) -> Option<&WzString> {
        match &self.object_type {
            WzObjectType::Value(WzValue::String(string)) |
            WzObjectType::Value(WzValue::UOL(string)) => Some(string),
            _ => None,
        }
    }

    try_as_wz_value!(try_as_lua, Lua, WzLua);
    try_as_wz_value!(try_as_raw_data, RawData, WzRawData);

    try_as_wz_value!(try_as_vector2d, Vector, Vector2D);
    try_as_wz_value!(try_as_short, Short, i16);
    try_as_wz_value!(try_as_int, Int, i32);
    try_as_wz_value!(try_as_long, Long, i64);
    try_as_wz_value!(try_as_float, Float, f32);
    try_as_wz_value!(try_as_double, Double, f64);
}

#[cfg(test)]
mod test {
    
    use super::*;
    use crate::WzReader;
    use crate::property::{WzSoundType, WzStringMeta};
    use memmap2::Mmap;
    use std::fs::OpenOptions;

    fn setup_wz_reader() -> Result<WzReader, std::io::Error> {
        let dir = tempfile::tempdir()?;
        let file_path = dir.path().join("test.wz");

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(file_path)?;

        file.set_len(200)?;

        let map = unsafe { Mmap::map(&file)? };

        Ok(WzReader::new(map))
    }

    #[test]
    fn try_as_file() {
        let reader = setup_wz_reader().unwrap();
        let file = WzFile {
            offset: 0,
            block_size: 0,
            is_parsed: false,
            reader: Arc::new(reader),
            wz_file_meta: Default::default(),
        };
        let node = WzNode::new("test".to_string(), WzObjectType::File(Box::new(file)), None);

        assert!(node.try_as_file().is_some());
        assert!(node.try_as_directory().is_none());
    }

    #[test]
    fn try_as_directory() {
        let reader = Arc::new(setup_wz_reader().unwrap());
        let wzdir = WzDirectory::new(0, 0, &reader, false);
        let node = WzNode::new("test".to_string(), WzObjectType::Directory(Box::new(wzdir)), None);

        assert!(node.try_as_directory().is_some());
        assert!(node.try_as_file().is_none());
    }
    
    #[test]
    fn try_as_image() {
        let reader = Arc::new(setup_wz_reader().unwrap());
        let wzimage = WzImage::new("test".to_string(), 0, 0, &reader);
        let node = WzNode::new("test".to_string(), WzObjectType::Image(Box::new(wzimage)), None);

        assert!(node.try_as_image().is_some());
        assert!(node.try_as_file().is_none());
    }

    #[test]
    fn try_as_sub_property() {
        let node = WzNode::new("test".to_string(), WzObjectType::Property(WzSubProperty::Property), None);

        assert!(node.try_as_sub_property().is_some());
        assert!(node.try_as_file().is_none());
    }
    #[test]
    fn try_as_value() {
        let node = WzNode::new("test".to_string(), WzObjectType::Value(WzValue::Null), None);

        assert!(node.try_as_value().is_some());
        assert!(node.try_as_file().is_none());
    }

    #[test]
    fn try_as_png() {
        let reader = Arc::new(setup_wz_reader().unwrap());
        let png = WzPng::new(&reader, (1, 1), (1, 1), (0, 1), 0);
        let node = WzNode::new("test".to_string(), WzObjectType::Property(WzSubProperty::PNG(Box::new(png))), None);

        assert!(node.try_as_png().is_some());
        assert!(node.try_as_file().is_none());
    }
    #[test]
    fn try_as_sound() {
        let reader = Arc::new(setup_wz_reader().unwrap());
        let png = WzSound::new(&reader, 0, 0, 0, 0, 0, WzSoundType::Mp3);
        let node = WzNode::new("test".to_string(), WzObjectType::Property(WzSubProperty::Sound(Box::new(png))), None);

        assert!(node.try_as_sound().is_some());
        assert!(node.try_as_file().is_none());
    }
    #[test]
    fn try_as_string() {
        let reader = Arc::new(setup_wz_reader().unwrap());
        let string = WzString::from_meta(WzStringMeta::empty(), &reader);
        let node = WzNode::new("test".to_string(), WzObjectType::Value(WzValue::String(string)), None);

        assert!(node.try_as_string().is_some());
        assert!(node.try_as_file().is_none());
    }

    #[test]
    fn try_as_string_uol() {
        let reader = Arc::new(setup_wz_reader().unwrap());
        let string = WzString::from_meta(WzStringMeta::empty(), &reader);
        let node = WzNode::new("test".to_string(), WzObjectType::Value(WzValue::UOL(string)), None);

        assert!(node.try_as_string().is_some());
        assert!(node.try_as_file().is_none());
    }
    #[test]
    fn try_as_lua() {
        let reader = Arc::new(setup_wz_reader().unwrap());
        let lua = WzLua::new(&reader, 0, 0);
        let node = WzNode::new("test".to_string(), WzObjectType::Value(WzValue::Lua(lua)), None);

        assert!(node.try_as_lua().is_some());
        assert!(node.try_as_file().is_none());
    }
    #[test]
    fn try_as_raw_data() {
        let reader = Arc::new(setup_wz_reader().unwrap());
        let raw_data = WzRawData::new(&reader, 0, 0);
        let node = WzNode::new("test".to_string(), WzObjectType::Value(WzValue::RawData(raw_data)), None);

        assert!(node.try_as_raw_data().is_some());
        assert!(node.try_as_file().is_none());
    }

    #[test]
    fn try_as_vector2d() {
        let vec2 = Vector2D::new(2, 3);
        let node = WzNode::new("test".to_string(), WzObjectType::Value(WzValue::Vector(vec2)), None);

        assert!(node.try_as_file().is_none());
        assert_eq!(node.try_as_vector2d(), Some(&Vector2D::new(2, 3)));
    }
    #[test]
    fn try_as_short() {
        let node = WzNode::new("test".to_string(), WzObjectType::Value(WzValue::Short(1)), None);

        assert!(node.try_as_file().is_none());
        assert_eq!(node.try_as_short(), Some(&1));
    }
    #[test]
    fn try_as_int() {
        let node = WzNode::new("test".to_string(), WzObjectType::Value(WzValue::Int(1)), None);

        assert!(node.try_as_file().is_none());
        assert_eq!(node.try_as_int(), Some(&1));
    }
    #[test]
    fn try_as_long() {
        let node = WzNode::new("test".to_string(), WzObjectType::Value(WzValue::Long(1)), None);

        assert!(node.try_as_file().is_none());
        assert_eq!(node.try_as_long(), Some(&1));
    }
    #[test]
    fn try_as_float() {
        let node = WzNode::new("test".to_string(), WzObjectType::Value(WzValue::Float(1.0)), None);

        assert!(node.try_as_file().is_none());
        assert_eq!(node.try_as_float(), Some(&1.0));
    }
    #[test]
    fn try_as_double() {
        let node = WzNode::new("test".to_string(), WzObjectType::Value(WzValue::Double(1.0)), None);

        assert!(node.try_as_file().is_none());
        assert_eq!(node.try_as_double(), Some(&1.0));
    }
}