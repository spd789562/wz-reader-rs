use std::sync::{Arc, RwLock, Weak, Mutex};
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use memmap2::Mmap;
use crate::Reader;
use crate::{ WzFileMeta, WzObjectType, WzReader, WzImageParseError, parse_wz_file, parse_wz_image, parse_wz_directory };
use crate::property::{WzPropertyType, png::WzPngParseError, string::WzStringParseError, sound::WzSoundParseError, lua::{WzLuaParseError, extract_lua}};
use crate::node::{NodeMethods, NodeParseError};
use image::DynamicImage;


pub type SyncWzNodeRef = RwLock<WzNode>;
pub type WzNodeArc = Arc<SyncWzNodeRef>;

#[derive(Debug, Clone)]
pub struct WzNode {
    pub reader: Option<Arc<WzReader>>,
    pub object_type: WzObjectType,
    pub property_type: WzPropertyType,
    pub name: String,
    pub offset: usize,
    pub block_size: usize,
    pub is_parsed: Option<Arc<Mutex<bool>>>,
    pub parent: Weak<RwLock<WzNode>>,
    pub children: HashMap<String, Arc<RwLock<WzNode>>>,

    /// wz_file_meta is only available for WzObjectType::File
    pub wz_file_meta: Option<WzFileMeta>,
}

impl WzNode {
    pub fn get_offset_range(&self) -> (usize, usize) {
        (self.offset, self.offset + self.block_size)
    }
}

impl NodeMethods for WzNodeArc {
    type Node = WzNodeArc;
    type Reader = Arc<WzReader>;
    fn new_wz_file(path: &str, parent: Option<&WzNodeArc>) -> WzNodeArc {
        let file: File = File::open(path).expect("file not found");
        let map = unsafe { Mmap::map(&file).unwrap() };
        let name = Path::new(path).file_stem().unwrap().to_str().unwrap().to_string();

        let block_size = map.len();
        let reader = WzReader::new(map);

        let offset = reader.get_wz_fstart().unwrap() + 2;

        let wz_file_meta = WzFileMeta {
            path: path.to_string(),
            name: name.clone(),
            patch_version: -1,
            wz_version_header: 0,
            wz_with_encrypt_version_header: true,
            hash: 0
        };

        let parent = match parent {
            Some(parent) => Arc::downgrade(parent),
            None => Weak::new()
        };

        Arc::new(RwLock::new(WzNode {
            object_type: WzObjectType::File,
            property_type: WzPropertyType::Null,
            offset: offset as usize,
            block_size,
            name,
            is_parsed: Some(Arc::new(Mutex::new(false))),
            parent,
            children: HashMap::new(),
            reader: Some(Arc::new(reader)),
            wz_file_meta: Some(wz_file_meta)
        }))
    }
    fn new_wz_img_file(path: &str, parent: Option<&WzNodeArc>) -> WzNodeArc {
        let file: File = File::open(path).expect("file not found");
        let map = unsafe { Mmap::map(&file).unwrap() };
        let name = Path::new(path).file_stem().unwrap().to_str().unwrap().to_string();

        let block_size = map.len();
        let reader = WzReader::new(map);

        let parent = match parent {
            Some(parent) => Arc::downgrade(parent),
            None => Weak::new()
        };

        Arc::new(RwLock::new(WzNode {
            object_type: WzObjectType::Image,
            property_type: WzPropertyType::Null,
            offset: 0,
            block_size,
            name,
            is_parsed: Some(Arc::new(Mutex::new(false))),
            parent,
            children: HashMap::new(),
            reader: Some(Arc::new(reader)),
            wz_file_meta: None,
        }))
    }
    fn new_empty_wz_directory(name: String, parent: Option<&WzNodeArc>) -> WzNodeArc {
        let parent = match parent {
            Some(parent) => Arc::downgrade(parent),
            None => Weak::new()
        };
        Arc::new(RwLock::new(WzNode {
            object_type: WzObjectType::Directory,
            property_type: WzPropertyType::Null,
            offset: 0,
            block_size: 0,
            name,
            is_parsed: None,
            parent,
            children: HashMap::new(),
            reader: None,
            wz_file_meta: None,
        }))
    }
    fn new_wz_directory(parent: &WzNodeArc, name: String, offset: usize, block_size: usize) -> WzNodeArc {
        Arc::new(RwLock::new(WzNode {
            object_type: WzObjectType::Directory,
            property_type: WzPropertyType::Null,
            offset,
            block_size,
            name,
            is_parsed: Some(Arc::new(Mutex::new(false))),
            parent: Arc::downgrade(parent),
            children: HashMap::new(),
            reader: parent.get_reader(),
            wz_file_meta: parent.read().unwrap().wz_file_meta.clone(),
        }))
    }
    fn new_wz_image(parent: &WzNodeArc, name: String, offset: usize, block_size: usize) -> WzNodeArc {
        Arc::new(RwLock::new(WzNode {
            object_type: WzObjectType::Image,
            property_type: WzPropertyType::Null,
            offset,
            block_size,
            name,
            is_parsed: Some(Arc::new(Mutex::new(false))),
            parent: Arc::downgrade(parent),
            children: HashMap::new(),
            reader: parent.get_reader(),
            wz_file_meta: parent.read().unwrap().wz_file_meta.clone(),
        }))
    }
    fn new_with_parent(parent: &WzNodeArc, object_type: WzObjectType, property_type: Option<WzPropertyType>, name: String, offset: usize, block_size: usize) -> WzNodeArc {
        Arc::new(RwLock::new(WzNode {
            object_type,
            property_type: property_type.unwrap_or(WzPropertyType::Null),
            offset,
            block_size,
            name,
            is_parsed: None,
            parent: Arc::downgrade(parent),
            children: HashMap::new(),
            reader: parent.get_reader(),
            wz_file_meta: None,
        }))
    }
    fn new_sub_property(parent: &WzNodeArc, name: String, offset: usize, block_size: usize) -> WzNodeArc {
        Arc::new(RwLock::new(WzNode {
            object_type: WzObjectType::Property,
            property_type: WzPropertyType::SubProperty,
            offset,
            block_size,
            name,
            is_parsed: None,
            parent: Arc::downgrade(parent),
            children: HashMap::new(),
            reader: parent.get_reader(),
            wz_file_meta: None,
        }))
    }
    fn new_wz_primitive_property(parent: &WzNodeArc, property_type: WzPropertyType, name: String) -> WzNodeArc {
        Arc::new(RwLock::new(WzNode {
            object_type: WzObjectType::Property,
            property_type,
            offset: 0,
            block_size: 0,
            name,
            is_parsed: None,
            parent: Arc::downgrade(parent),
            children: HashMap::new(),
            reader: None,
            wz_file_meta: None,
        }))
    }

    fn get_reader(&self) -> Option<Arc<WzReader>> {
        let node = self.read().unwrap();
        node.reader.clone()
    }

    
    fn first_image(&self) ->Result<WzNodeArc, NodeParseError> {
        let node = self.read().unwrap();
        if let Some(node) = node.children.values().find(|node| node.read().unwrap().object_type == WzObjectType::Image) {
            Ok(Arc::clone(node))
        } else {
            Err(NodeParseError::NodeNotFound)
        }
    }
    fn at(&self, name: &str) -> Result<WzNodeArc, NodeParseError> {
        let node = self.read().unwrap();
        if let Some(node) = node.children.get(name) {
            Ok(Arc::clone(node))
        } else {
            Err(NodeParseError::NodeNotFound)
        }
    }
    fn at_path(&self, path: &str, force_parse: bool) -> Result<WzNodeArc, NodeParseError> {
        let mut current_node = self.clone();
        for name in path.split('/') {
            if force_parse {
                current_node.parse().unwrap();
            }
            current_node = {
                match current_node.at(name) {
                    Ok(node) => node,
                    _ => return Err(NodeParseError::NodeNotFound)
                }
            };
        }
        Ok(current_node)
    }
    fn at_path_unchecked(&self, path: &str) -> Result<WzNodeArc, NodeParseError> {
        let mut current_node = self.clone();
        for name in path.split('/') {
            current_node = {
                match current_node.at(name) {
                    Ok(node) => node,
                    _ => return Err(NodeParseError::NodeNotFound)
                }
            };
        }
        Ok(current_node)
    }
    fn get_parent_wz_image(&self) -> Result<WzNodeArc, NodeParseError> {
        let mut current_node = self.clone();
        loop {
            current_node = {
                let node = current_node.read().unwrap();
                match node.parent.upgrade() {
                    Some(parent) => {
                        if parent.read().unwrap().object_type == WzObjectType::Image {
                            break Ok(parent);
                        }
                        parent
                    },
                    None => {
                        break Err(NodeParseError::NodeNotFound)
                    }
                }
            };
        }
    }
    fn get_base_wz_file(&self) -> Result<WzNodeArc, NodeParseError> {
        let mut current_node = self.clone();
        loop {
            current_node = {
                let node = current_node.read().unwrap();
                match node.parent.upgrade() {
                    Some(parent) => {
                        {
                            let parent_read = parent.read().unwrap();
                            if parent_read.object_type == WzObjectType::File && parent_read.name.as_str() == "Base" {
                                break Ok(parent.clone());
                            }
                        }
                        parent
                    },
                    None => {
                        break Err(NodeParseError::NodeNotFound)
                    }
                }
            };
        }
    }
    fn get_uol_wz_node(&self) -> Result<WzNodeArc, NodeParseError> {
        let node = self.read().unwrap();
        match &node.property_type {
            WzPropertyType::UOL(meta) => {
                if let Some(reader) = &node.reader {
                    let path = reader.resolve_wz_string_meta(meta).map_err(WzStringParseError::from).unwrap();
                    self.resolve_relative_path(&format!("../{path}"), true)
                } else {
                    panic!("WzReader not found in WzPropertyType::UOL")
                }
            },
            _ => panic!("This method only available for WzPropertyType::UOL")
        }
    }
    
    fn get_name(&self) -> String {
        self.read().unwrap().name.clone()
    }
    fn get_offset(&self) -> usize {
        self.read().unwrap().offset
    }
    fn get_block_size(&self) -> usize {
        self.read().unwrap().block_size
    }
    fn get_wz_file_hash(&self) -> Option<usize> {
        self.read().unwrap().wz_file_meta.as_ref().map(|meta| meta.hash)
    }
    fn get_full_path(&self) -> String {
        let mut current_node = self.clone();
        let mut path = String::new();
        loop {
            current_node = {
                let node = current_node.read().unwrap();
                if path.is_empty() {
                    path = node.name.clone();
                } else {
                    path = format!("{}/{}", node.name, path);
                }
                match node.parent.upgrade() {
                    Some(parent) => {
                        parent
                    },
                    None => {
                        break
                    }
                }
            };
        }
        path
    }
    fn resolve_relative_path(&self, path: &str, force_parse: bool) -> Result<WzNodeArc, NodeParseError> {
        let mut current_node = self.clone();

        
        for name in path.split('/') {
            if name == ".." {
                current_node = {
                    let node = current_node.read().unwrap();
                    node.parent.upgrade().unwrap()
                };
            } else {
                if force_parse {
                    current_node.parse().unwrap();
                }

                current_node = {
                    match current_node.at(name) {
                        Ok(node) => node,
                        _ => return Err(NodeParseError::NodeNotFound)
                    }
                };
            }
        }
        Ok(current_node)
    }

    fn update_parse_status(&self, status: bool) {
        if let Some(is_parsed) = self.read().unwrap().is_parsed.clone() {
            let mut is_parsed = is_parsed.lock().unwrap();
            *is_parsed = status;
        }
    }
    fn update_wz_file_meta(&self, wz_file_meta: WzFileMeta) {
        let mut node = self.write().unwrap();
        if let Some(meta) = &node.wz_file_meta {
            node.wz_file_meta = Some(WzFileMeta {
                path: meta.path.clone(),
                name: meta.name.clone(),
                patch_version: wz_file_meta.patch_version,
                wz_version_header: wz_file_meta.wz_version_header,
                wz_with_encrypt_version_header: wz_file_meta.wz_with_encrypt_version_header,
                hash: wz_file_meta.hash
            });
        } else {
            node.wz_file_meta = Some(wz_file_meta);
        }
    }
    fn update_wz_png_meta(&self, name: String, offset: usize, block_size: usize, property_type: WzPropertyType) {
        let mut node = self.write().unwrap();
        node.name = name;
        node.property_type = property_type;
        node.offset = offset;
        node.block_size = block_size;
    }
    
    fn transfer_childs(&self, to: &WzNodeArc) {
        let mut node = self.write().unwrap();
        node.children.values().for_each(|child| {
            {
                let mut child = child.write().unwrap();
                child.parent = Arc::downgrade(to);
            }
            to.add_node_child(Arc::clone(child));
        });
        node.children.clear();
    }
    fn add_node_child(&self, child: WzNodeArc) {
        let mut node = self.write().unwrap();
        let child_name = child.get_name();
        node.children.insert(child_name, child);
    }

    fn unparse_image(&self) -> Result<(), NodeParseError> {
        let node = self.write();

        if let Ok(mut node) = node {
            if node.object_type != WzObjectType::Image {
                return Err(NodeParseError::from(WzImageParseError::NotImageObject));
            }

            let guard = node.is_parsed.clone();

            if let Some(guard) = guard {
                let mut guard = guard.lock().unwrap();
                
                if !*guard {
                    return Ok(());
                }

                node.children.clear();
                *guard = false;
            }

            Ok(())
        } else {
            Err(NodeParseError::NodeHasBeenUsing)
        }
    }
    fn parse(&self) -> Result<(), NodeParseError> {
        let obejct_type = {
            let node = self.read().unwrap();

            if let Some(guard) = node.is_parsed.clone() {
                let guard = guard.lock().unwrap();
                if *guard {
                    return Ok(());
                }
            }

            node.object_type
        };

        match obejct_type {
            WzObjectType::File => self.parse_wz_file(None),
            WzObjectType::Directory => self.parse_wz_directory(),
            WzObjectType::Image => self.parse_wz_image(),
            _ => Ok(())
        }
    }
    fn parse_wz_image(&self) -> Result<(), NodeParseError> {

        let guard = self.read().unwrap().is_parsed.clone();

        if let Some(guard) = guard {
            let mut guard = guard.lock().unwrap();
            if *guard {
                return Ok(());
            }

            if let Err(e) = parse_wz_image(self) {
                return Err(NodeParseError::from(e));
            }

            *guard = true;
        }

        Ok(())
    }

    fn parse_wz_directory(&self) -> Result<(), NodeParseError> {
        let guard = self.read().unwrap().is_parsed.clone();
        if let Some(guard) = guard {
            let mut guard = guard.lock().unwrap();
            if *guard {
                return Ok(());
            }

            if let Err(e) = parse_wz_directory(self) {
                return Err(NodeParseError::from(e));
            }

            *guard = true;
        }

        Ok(())
    }
    fn parse_wz_file(&self, patch_verions: Option<i32>) -> Result<(), NodeParseError> {
        let guard = self.read().unwrap().is_parsed.clone();
        if let Some(guard) = guard {
            let mut guard = guard.lock().unwrap();
            if *guard {
                return Ok(());
            }

            if let Err(e) = parse_wz_file(self, patch_verions) {
                return Err(NodeParseError::from(e));
            }

            *guard = true;
        }

        Ok(())
    }

    fn is_end(&self) -> bool {
        self.read().unwrap().children.is_empty()
    }
    fn is_png(&self) -> bool {
        matches!(self.read().unwrap().property_type, WzPropertyType::PNG(_))
    }
    fn is_sound(&self) -> bool {
        matches!(self.read().unwrap().property_type, WzPropertyType::Sound(_))
    }
    fn is_string(&self) -> bool {
        matches!(self.read().unwrap().property_type, WzPropertyType::String(_))
    }
    fn is_lua(&self) -> bool {
        matches!(self.read().unwrap().property_type, WzPropertyType::Lua)
    }
    fn is_uol(&self) -> bool {
        matches!(self.read().unwrap().property_type, WzPropertyType::UOL(_))
    }

    fn get_string(&self) -> Result<String, WzStringParseError> {
        let node = self.read().unwrap();
        match &node.property_type {
            WzPropertyType::String(meta) | WzPropertyType::UOL(meta) => {
                if let Some(reader) = &node.reader {
                    reader.resolve_wz_string_meta(meta).map_err(WzStringParseError::from)
                } else {
                    panic!("WzReader not found in WzPropertyType::String")
                }
            },
            _ => Err(WzStringParseError::NotStringProperty)
        }
    }
    fn get_image(&self) -> Result<DynamicImage, WzPngParseError> {
        let node = self.read().unwrap();

        match &node.property_type {
            WzPropertyType::PNG(png) => {
                if let Ok(inlink) = self.at("_inlink") {
                    if let Ok(parent_node) = self.get_parent_wz_image() {
                        let path = inlink.get_string().unwrap();
                        if let Ok(target) = parent_node.at_path(&path, false) {
                            return target.get_image();
                        }
                        return Err(WzPngParseError::LinkError);
                    }
                    return Err(WzPngParseError::LinkError);
                }
                if let Ok(outlink) = self.at("_outlink") {
                    /* outlink always resolve from base */
                    if let Ok(base_node) = self.get_base_wz_file() {
                        let path = outlink.get_string().unwrap();
                        if let Ok(target) = base_node.at_path(&path, true) {
                            return target.get_image();
                        }
                        return Err(WzPngParseError::LinkError);
                    }
                    return Err(WzPngParseError::LinkError);
                }
                /* if don't have both _inlink or _outlink */
                if let Some(reader) = &node.reader {
                    let buffer = reader.get_slice(node.get_offset_range());
                    png.extract_png(buffer)
                } else {
                    panic!("WzReader not found in WzPropertyType::PNG, maybe is a bug")
                }
            },
            _ => Err(WzPngParseError::NotPngProperty)
        }
    }
    fn save_image(&self, path: &str, name: Option<&str>) -> Result<(), WzPngParseError> {
        if self.is_png() {
            let image = self.get_image()?;
            let path = Path::new(path).join(name.unwrap_or(&self.get_name())).with_extension(".png");
            image.save(path).map_err(WzPngParseError::from)
        } else {
            Err(WzPngParseError::NotPngProperty)
        }
    }
    fn get_sound(&self) -> Result<Vec<u8>, WzSoundParseError> {
        let node = self.read().unwrap();
        match &node.property_type {
            WzPropertyType::Sound(meta) => {
                if let Some(reader) = &node.reader {
                    Ok(meta.get_buffer(&reader.map))
                } else {
                    panic!("WzReader not found in WzPropertyType::Sound")
                }
            },
            _ => Err(WzSoundParseError::NotSoundProperty)
        }
    }
    fn save_sound(&self, path: &str, name: Option<&str>) -> Result<(), WzSoundParseError> {
        let node = self.read().unwrap();
        match &node.property_type {
            WzPropertyType::Sound(sound) => {
                if let Some(reader) = &node.reader {
                    let path = Path::new(path).join(name.unwrap_or(&node.name));
                    sound.extract_sound(&reader.map, path)
                } else {
                    panic!("WzReader not found in WzPropertyType::Sound")
                }
            },
            _ => Err(WzSoundParseError::NotSoundProperty)
        }
    }
    fn get_lua(&self) -> Result<String, WzLuaParseError> {
        let node = self.read().unwrap();
        match &node.property_type {
            WzPropertyType::Lua => {
                if let Some(reader) = &node.reader {
                    extract_lua(reader.get_slice(node.get_offset_range()))
                } else {
                    panic!("WzReader not found in WzPropertyType::Lua")
                }
            },
            _ => Err(WzLuaParseError::NotLuaProperty)
        }
    }
}