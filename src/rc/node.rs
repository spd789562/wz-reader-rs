use std::rc::{Rc, Weak};
use core::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use memmap2::Mmap;
use crate::Reader;
use crate::{ WzFileMeta, WzObjectType, property::WzPropertyType, property::png::WzPngParseError, WzReader, parse_wz_file, parse_wz_image, parse_wz_directory };
use crate::node::{NodeMethods, NodeParseError};
use image::DynamicImage;

pub type WzNodeRef = RefCell<WzNode>;
pub type WzNodeRc = Rc<WzNodeRef>;

#[derive(Debug, Clone)]
pub struct WzNode {
    pub reader: Option<Rc<WzReader>>,
    pub object_type: WzObjectType,
    pub property_type: Option<WzPropertyType>,
    pub name: String,
    pub offset: usize,
    pub block_size: usize,
    pub is_parsed: bool,
    pub parent: Weak<WzNodeRef>,
    pub children: HashMap<String, WzNodeRc>,

    /// wz_file_meta is only available for WzObjectType::File
    pub wz_file_meta: Option<WzFileMeta>,
}

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

impl NodeMethods for WzNodeRc {
    type Node = WzNodeRc;
    type Reader = Rc<WzReader>;
    fn new_wz_file(path: &str, parent: Option<&WzNodeRc>) -> WzNodeRc {
        let file: File = File::open(path).expect("file not found");
        let map = unsafe { Mmap::map(&file).unwrap() };
        let name = Path::new(path).file_name().unwrap().to_str().unwrap().to_string();

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
            Some(parent) => Rc::downgrade(parent),
            None => Weak::new()
        };

        Rc::new(RefCell::new(WzNode {
            object_type: WzObjectType::File,
            property_type: None,
            offset: offset as usize,
            block_size,
            name,
            is_parsed: false,
            parent,
            children: HashMap::new(),
            reader: Some(Rc::new(reader)),
            wz_file_meta: Some(wz_file_meta),
        }))
    }
    fn new(object_type: WzObjectType, property_type: Option<WzPropertyType>, name: String, offset: usize, block_size: usize) -> WzNodeRc {
        Rc::new(RefCell::new(WzNode {
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
    fn new_empty_wz_directory(name: String, parent: Option<&WzNodeRc>) -> WzNodeRc {
        let reader = parent.and_then(|parent| parent.get_reader());
        let parent = match parent {
            Some(parent) => Rc::downgrade(parent),
            None => Weak::new()
        };
        Rc::new(RefCell::new(WzNode {
            object_type: WzObjectType::Directory,
            property_type: None,
            offset: 0,
            block_size: 0,
            name,
            is_parsed: true,
            parent,
            children: HashMap::new(),
            reader,
            wz_file_meta: None
        }))
    }
    fn new_with_parent(parent: &WzNodeRc, object_type: WzObjectType, property_type: Option<WzPropertyType>, name: String, offset: usize, block_size: usize) -> WzNodeRc {
        Rc::new(RefCell::new(WzNode {
            object_type,
            property_type,
            offset,
            block_size,
            name,
            is_parsed: false,
            parent: Rc::downgrade(parent),
            children: HashMap::new(),
            reader: parent.get_reader(),
            wz_file_meta: None
        }))
    }
    fn new_sub_property(parent: &WzNodeRc, name: String, offset: usize, block_size: usize) -> WzNodeRc {
        Rc::new(RefCell::new(WzNode {
            object_type: WzObjectType::Property,
            property_type: Some(WzPropertyType::SubProperty),
            offset,
            block_size,
            name,
            is_parsed: true,
            parent: Rc::downgrade(parent),
            children: HashMap::new(),
            reader: parent.get_reader(),
            wz_file_meta: None
        }))
    }
    fn new_wz_primitive_property(parent: &WzNodeRc, property_type: Option<WzPropertyType>, name: String) -> WzNodeRc {
        Rc::new(RefCell::new(WzNode {
            object_type: WzObjectType::Property,
            property_type,
            offset: 0,
            block_size: 0,
            name,
            is_parsed: true,
            parent: Rc::downgrade(parent),
            children: HashMap::new(),
            reader: None,
            wz_file_meta: None
        }))
    }

    fn get_reader(&self) -> Option<Rc<WzReader>> {
        self.borrow().reader.clone()
    }

    fn first_image(&self) -> Option<WzNodeRc> {
        self.borrow().children.iter().find(|node| node.1.borrow().object_type == WzObjectType::Image).map(|(_, node)| Rc::clone(node))
    }
    fn at(&self, name: &str) -> Option<WzNodeRc> {
        self.borrow().children.get(name).map(Rc::clone)
    }
    fn at_path(&self, path: &str) -> Option<WzNodeRc> {
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
        self.borrow().name.clone()
    }
    fn get_offset(&self) -> usize {
        self.borrow().offset
    }
    fn get_block_size(&self) -> usize {
        self.borrow().block_size
    }
    fn get_full_path(&self) -> String {
        let mut current_node = self.clone();
        let mut path = String::new();
        loop {
            current_node = {
                let node = current_node.borrow();
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
    fn resolve_relative_path(&self, path: &str, force_parse: bool) -> Option<WzNodeRc> {
        let mut current_node = self.clone();

        
        for name in path.split('/') {
            if name == ".." {
                current_node = {
                    let node = current_node.borrow();
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
        let mut node = self.borrow_mut();
        node.is_parsed = status;
    }
    fn update_wz_file_meta(&self, wz_file_meta: WzFileMeta) {
        let mut node = self.borrow_mut();
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
        let mut node = self.borrow_mut();
        node.name = name;
        node.property_type = Some(property_type);
        node.offset = offset;
        node.block_size = block_size;
    }
    fn transfer_childs(&self, to: &WzNodeRc) {
        let mut node = self.borrow_mut();
        node.children.values().for_each(|child| {
            {
                let mut child = child.borrow_mut();
                child.parent = Rc::downgrade(to);
            }
            to.add_node_child(Rc::clone(child));
        });
        node.children.clear();
    }
    fn add_node_child(&self, child: WzNodeRc) {
        let mut node = self.borrow_mut();
        let child_name = child.get_name();
        node.children.insert(child_name, child);
    }
    fn parse(&self) -> Result<(), NodeParseError> {
        let obejct_type = {
            let node = self.borrow();

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