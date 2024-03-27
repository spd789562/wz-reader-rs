use std::rc::{Rc, Weak};
use core::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use memmap2::Mmap;
use crate::Reader;
use crate::{ WzFileMeta, WzObjectType, WzReader, WzImageParseError, parse_wz_file, parse_wz_image, parse_wz_directory };
use crate::property::{WzPropertyType, png::WzPngParseError, string::WzStringParseError, sound::WzSoundParseError, lua::{WzLuaParseError, extract_lua}};
use crate::node::{NodeMethods, NodeParseError};
use image::DynamicImage;

pub type WzNodeRef = RefCell<WzNode>;
pub type WzNodeRc = Rc<WzNodeRef>;

#[derive(Debug, Clone)]
pub struct WzNode {
    pub reader: Option<Rc<WzReader>>,
    pub object_type: WzObjectType,
    pub property_type: WzPropertyType,
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
            property_type: WzPropertyType::Null,
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
    fn new_wz_img_file(path: &str, parent: Option<&WzNodeRc>) -> WzNodeRc {
        let file: File = File::open(path).expect("file not found");
        let map = unsafe { Mmap::map(&file).unwrap() };
        let name = Path::new(path).file_stem().unwrap().to_str().unwrap().to_string();

        let block_size = map.len();
        let reader = WzReader::new(map);

        let parent = match parent {
            Some(parent) => Rc::downgrade(parent),
            None => Weak::new()
        };

        Rc::new(RefCell::new(WzNode {
            object_type: WzObjectType::Image,
            property_type: WzPropertyType::Null,
            offset: 0,
            block_size,
            name,
            is_parsed: false,
            parent,
            children: HashMap::new(),
            reader: Some(Rc::new(reader)),
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
            property_type: WzPropertyType::Null,
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
    fn new_wz_directory(parent: &WzNodeRc, name: String, offset: usize, block_size: usize) -> WzNodeRc {
        Rc::new(RefCell::new(WzNode {
            object_type: WzObjectType::Directory,
            property_type: WzPropertyType::Null,
            offset,
            block_size,
            name,
            is_parsed: false,
            parent: Rc::downgrade(parent),
            children: HashMap::new(),
            reader: parent.get_reader(),
            wz_file_meta: parent.borrow().wz_file_meta.clone()
        }))
    }
    fn new_wz_image(parent: &WzNodeRc, name: String, offset: usize, block_size: usize) -> WzNodeRc {
        Rc::new(RefCell::new(WzNode {
            object_type: WzObjectType::Image,
            property_type: WzPropertyType::Null,
            offset,
            block_size,
            name,
            is_parsed: false,
            parent: Rc::downgrade(parent),
            children: HashMap::new(),
            reader: parent.get_reader(),
            wz_file_meta: parent.borrow().wz_file_meta.clone()
        }))
    }
    fn new_with_parent(parent: &WzNodeRc, object_type: WzObjectType, property_type: Option<WzPropertyType>, name: String, offset: usize, block_size: usize) -> WzNodeRc {
        Rc::new(RefCell::new(WzNode {
            object_type,
            property_type: property_type.unwrap_or(WzPropertyType::Null),
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
            property_type: WzPropertyType::SubProperty,
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
    fn new_wz_primitive_property(parent: &WzNodeRc, property_type: WzPropertyType, name: String) -> WzNodeRc {
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

    fn first_image(&self) -> Result<WzNodeRc, NodeParseError> {
        if let Some(node) = self.borrow().children.values().find(|node| node.borrow().object_type == WzObjectType::Image) {
            Ok(Rc::clone(node))
        } else {
            Err(NodeParseError::NodeNotFound)
        }
    }
    fn at(&self, name: &str) -> Result<WzNodeRc, NodeParseError> {
        if let Some(node) = self.borrow().children.get(name) {
            Ok(Rc::clone(node))
        } else {
            Err(NodeParseError::NodeNotFound)
        }
    }
    fn at_path(&self, path: &str, force_parse: bool) -> Result<WzNodeRc, NodeParseError> {
        path.split('/').try_fold(self.clone(), |node, name| {
            if force_parse {
                node.parse()?;
            }
            node.at(name)
        })
    }
    fn at_path_unchecked(&self, path: &str) -> Result<WzNodeRc, NodeParseError> {
        path.split('/').try_fold(self.clone(), |node, name| node.at(name))
    }
    fn get_parent_wz_image(&self) -> Result<WzNodeRc, NodeParseError> {
        let mut current_node = self.clone();
        loop {
            current_node = {
                let node = current_node.borrow();
                match node.parent.upgrade() {
                    Some(parent) => {
                        if parent.borrow().object_type == WzObjectType::Image {
                            return Ok(parent);
                        }
                        parent
                    },
                    None => {
                        return Err(NodeParseError::NodeNotFound)
                    }
                }
            };
        }
    }
    fn get_base_wz_file(&self) -> Result<WzNodeRc, NodeParseError> {
        let mut current_node = self.clone();
        loop {
            current_node = {
                let node = current_node.borrow();
                match node.parent.upgrade() {
                    Some(parent) => {
                        {
                            let parent_read = parent.borrow();
                            if parent_read.object_type == WzObjectType::File && parent_read.name.as_str() == "Base" {
                                return Ok(parent.clone());
                            }
                        }
                        parent
                    },
                    None => {
                        return Err(NodeParseError::NodeNotFound)
                    }
                }
            };
        }
    }
    fn get_uol_wz_node(&self) -> Result<WzNodeRc, NodeParseError> {
        let node = self.borrow();
        match &node.property_type {
            WzPropertyType::UOL(meta) => {
                if let Some(reader) = &node.reader {
                    let path = reader.resolve_wz_string_meta(meta, node.offset, node.block_size).map_err(WzStringParseError::from).unwrap();
                    self.resolve_relative_path(&format!("../{path}"), true)
                } else {
                    panic!("WzReader not found in WzPropertyType::UOL")
                }
            },
            _ => panic!("This method only available for WzPropertyType::UOL")
        }
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
    fn get_wz_file_hash(&self) -> Option<usize> {
        self.borrow().wz_file_meta.as_ref().map(|meta| meta.hash)
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
    fn resolve_relative_path(&self, path: &str, force_parse: bool) -> Result<WzNodeRc, NodeParseError> {
        path.split('/')
            .try_fold(self.clone(), |node, name| {
                if name == ".." {
                    return node.borrow().parent.upgrade().ok_or(NodeParseError::NodeNotFound);
                }
                if force_parse {
                    node.parse()?;
                }
                node.at(name)
            })
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
    fn update_wz_png_meta(&self, offset: usize, block_size: usize, property_type: WzPropertyType) {
        let mut node = self.borrow_mut();
        node.property_type = property_type;
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

    fn unparse_image(&self) -> Result<(), NodeParseError> {
        let node = self.try_borrow_mut();

        if let Ok(mut node) = node {
            if !node.is_parsed {
                return Ok(());
            }
            if node.object_type != WzObjectType::Image {
                return Err(NodeParseError::from(WzImageParseError::NotImageObject));
            }
            node.children.clear();
            node.is_parsed = false;

            Ok(())
        } else {
            Err(NodeParseError::NodeHasBeenUsing)
        }
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

    fn is_end(&self) -> bool {
        self.borrow().children.is_empty()
    }
    fn is_png(&self) -> bool {
        matches!(self.borrow().property_type, WzPropertyType::PNG(_))
    }
    fn is_sound(&self) -> bool {
        matches!(self.borrow().property_type, WzPropertyType::Sound(_))
    }
    fn is_string(&self) -> bool {
        matches!(self.borrow().property_type, WzPropertyType::String(_))
    }
    fn is_lua(&self) -> bool {
        matches!(self.borrow().property_type, WzPropertyType::Lua)
    }
    fn is_uol(&self) -> bool {
        matches!(self.borrow().property_type, WzPropertyType::UOL(_))
    }


    fn get_string(&self) -> Result<String, WzStringParseError> {
        let node = self.borrow();
        match &node.property_type {
            WzPropertyType::String(meta) | WzPropertyType::UOL(meta) => {
                if let Some(reader) = &node.reader {
                    reader.resolve_wz_string_meta(meta, node.offset, node.block_size).map_err(WzStringParseError::from)
                } else {
                    panic!("WzReader not found in WzPropertyType::String")
                }
            },
            _ => Err(WzStringParseError::NotStringProperty)
        }
    }
    fn get_image(&self) -> Result<DynamicImage, WzPngParseError> {
        let node = self.borrow();
        match &node.property_type {
            WzPropertyType::PNG(png) => {
                if let Ok(inlink) = self.at("_inlink") {
                    if let Ok(parent_node) = self.get_parent_wz_image() {
                        let path = inlink.get_string().map_err(|_| WzPngParseError::LinkError)?;
                        if let Ok(target) = parent_node.at_path(&path, false) {
                            return target.get_image();
                        }
                    }
                    return Err(WzPngParseError::LinkError);
                }
                if let Ok(outlink) = self.at("_outlink") {
                    /* outlink always resolve from base */
                    if let Ok(base_node) = self.get_base_wz_file() {
                        let path = outlink.get_string().map_err(|_| WzPngParseError::LinkError)?;
                        if let Ok(target) = base_node.at_path(&path, true) {
                            return target.get_image();
                        }
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
        let node = self.borrow();
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
        let node = self.borrow();
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
        let node = self.borrow();
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
