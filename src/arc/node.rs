use std::sync::{Arc, RwLock, Weak};
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use memmap2::Mmap;
use crate::Reader;
use crate::{ WzFileMeta, WzObjectType, property::WzPropertyType, property::png::WzPngParseError, WzReader, parse_wz_file, parse_wz_image, parse_wz_directory };
use crate::node::{NodeMethods, NodeParseError};
use image::DynamicImage;


pub type SyncWzNodeRef = RwLock<WzNode>;
pub type WzNodeArc = Arc<SyncWzNodeRef>;

#[derive(Debug, Clone)]
pub struct WzNode {
    pub reader: Option<Arc<WzReader>>,
    pub object_type: WzObjectType,
    pub property_type: Option<WzPropertyType>,
    pub name: String,
    pub offset: usize,
    pub block_size: usize,
    pub is_parsed: bool,
    pub parent: Weak<RwLock<WzNode>>,
    pub children: HashMap<String, Arc<RwLock<WzNode>>>,

    /// wz_file_meta is only available for WzObjectType::File
    pub wz_file_meta: Option<WzFileMeta>,
}
unsafe impl Send for WzNode {}
unsafe impl Sync for WzNode {}


impl WzNode {
    pub fn get_offset_range(&self) -> (usize, usize) {
        (self.offset, self.offset + self.block_size)
    }

    pub fn resolve_string(&self) -> Result<String, String> {
        match &self.property_type {
            Some(WzPropertyType::String(meta)) => {
                if let Some(reader) = &self.reader {
                    Ok(reader.resolve_wz_string_meta(meta).unwrap())
                } else {
                    panic!("WzReader not found in WzPropertyType::String")
                }
            },
            _ => Err("Not a string property".to_string())
        }
    }
    pub fn resolve_png(&self) -> Result<DynamicImage, WzPngParseError> {
        match &self.property_type {
            Some(WzPropertyType::PNG(png)) => {
                if let Some(reader) = &self.reader {
                    let buffer = reader.get_slice(self.get_offset_range());
                    png.extract_png(buffer)
                } else {
                    Err(WzPngParseError::NotPngProperty)
                }
            },
            _ => Err(WzPngParseError::NotPngProperty)
        }
    }
    pub fn is_png(&self) -> bool {
        matches!(&self.property_type, Some(WzPropertyType::PNG(_)))
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
            property_type: None,
            offset: offset as usize,
            block_size,
            name,
            is_parsed: false,
            parent,
            children: HashMap::new(),
            reader: Some(Arc::new(reader)),
            wz_file_meta: Some(wz_file_meta),
        }))
    }
    fn new(object_type: WzObjectType, property_type: Option<WzPropertyType>, name: String, offset: usize, block_size: usize) -> WzNodeArc {
        Arc::new(RwLock::new(WzNode {
            object_type,
            property_type,
            offset,
            block_size,
            name,
            is_parsed: false,
            parent: Weak::new(),
            children: HashMap::new(),
            reader: None,
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
            property_type: None,
            offset: 0,
            block_size: 0,
            name,
            is_parsed: true,
            parent,
            children: HashMap::new(),
            reader: None,
            wz_file_meta: None
        }))
    }
    fn new_with_parent(parent: &WzNodeArc, object_type: WzObjectType, property_type: Option<WzPropertyType>, name: String, offset: usize, block_size: usize) -> WzNodeArc {
        Arc::new(RwLock::new(WzNode {
            object_type,
            property_type,
            offset,
            block_size,
            name,
            is_parsed: false,
            parent: Arc::downgrade(parent),
            children: HashMap::new(),
            reader: parent.get_reader(),
            wz_file_meta: None
        }))
    }
    fn new_sub_property(parent: &WzNodeArc, name: String, offset: usize, block_size: usize) -> WzNodeArc {
        Arc::new(RwLock::new(WzNode {
            object_type: WzObjectType::Property,
            property_type: Some(WzPropertyType::SubProperty),
            offset,
            block_size,
            name,
            is_parsed: true,
            parent: Arc::downgrade(parent),
            children: HashMap::new(),
            reader: parent.get_reader(),
            wz_file_meta: None
        }))
    }
    fn new_wz_primitive_property(parent: &WzNodeArc, property_type: Option<WzPropertyType>, name: String) -> WzNodeArc {
        Arc::new(RwLock::new(WzNode {
            object_type: WzObjectType::Property,
            property_type,
            offset: 0,
            block_size: 0,
            name,
            is_parsed: true,
            parent: Arc::downgrade(parent),
            children: HashMap::new(),
            reader: None,
            wz_file_meta: None
        }))
    }

    fn get_reader(&self) -> Option<Arc<WzReader>> {
        let node = self.read().unwrap();
        node.reader.clone()
    }

    
    fn first_image(&self) -> Option<WzNodeArc> {
        let node = self.read().unwrap();
        node.children.iter().find(|node| node.1.read().unwrap().object_type == WzObjectType::Image).map(|(_, node)| Arc::clone(node))
    }
    fn at(&self, name: &str) -> Option<WzNodeArc> {
        let node = self.read().unwrap();
        node.children.get(name).map(Arc::clone)
    }
    fn at_path(&self, path: &str) -> Option<WzNodeArc> {
        let mut current_node = self.clone();
        for name in path.split('/') {
            current_node = {
                match current_node.at(name) {
                    Some(node) => node,
                    None => return None
                }
            };
        }
        Some(current_node)
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
    fn resolve_relative_path(&self, path: &str, force_parse: bool) -> Option<WzNodeArc> {
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
                        Some(node) => node,
                        None => return None
                    }
                };
            }
        }
        Some(current_node)
    }

    fn update_parse_status(&self, status: bool) {
        let mut node = self.write().unwrap();
        node.is_parsed = status;
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
        node.property_type = Some(property_type);
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
    fn parse(&self) -> Result<(), NodeParseError> {
        let obejct_type = {
            let node = self.read().unwrap();

            if node.is_parsed {
                return Ok(());
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
        if let Err(e) = parse_wz_image(self) {
            return Err(NodeParseError::from(e));
        }

        self.update_parse_status(true);

        Ok(())
    }

    fn parse_wz_directory(&self) -> Result<(), NodeParseError> {
        if let Err(e) = parse_wz_directory(self) {
            return Err(NodeParseError::from(e));
        }

        self.update_parse_status(true);

        Ok(())
    }
    fn parse_wz_file(&self, patch_verions: Option<i32>) -> Result<(), NodeParseError> {
        if let Err(e) = parse_wz_file(self, patch_verions) {
            return Err(NodeParseError::from(e));
        }

        self.update_parse_status(true);

        Ok(())
    }
}