use crate::{ WzFileMeta, WzObjectType, WzDirectoryParseError, WzFileParseError, WzImageParseError};
use crate::property::{WzPropertyType, png::WzPngParseError, string::WzStringParseError, sound::WzSoundParseError};
use image::DynamicImage;
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
}

pub trait NodeMethods {
    type Reader;
    type Node;

    fn get_reader(&self) -> Option<Self::Reader>;

    fn new_wz_file(path: &str, parent: Option<&Self::Node>) -> Self::Node;
    fn new(object_type: WzObjectType, property_type: Option<WzPropertyType>, name: String, offset: usize, block_size: usize) -> Self::Node;
    fn new_empty_wz_directory(name: String, parent: Option<&Self::Node>) -> Self::Node;
    fn new_with_parent(parent: &Self::Node, object_type: WzObjectType, property_type: Option<WzPropertyType>, name: String, offset: usize, block_size: usize) -> Self::Node;
    fn new_sub_property(parent: &Self::Node, name: String, offset: usize, block_size: usize) -> Self::Node;
    fn new_wz_primitive_property(parent: &Self::Node, property_type: WzPropertyType, name: String) -> Self::Node;

    fn first_image(&self) -> Option<Self::Node>;
    fn at(&self, name: &str) -> Option<Self::Node>;
    fn at_path(&self, path: &str, force_parse: bool) -> Option<Self::Node>;
    fn get_parent_wz_image(&self) -> Option<Self::Node>;
    fn get_base_wz_file(&self) -> Option<Self::Node>;
    fn get_uol_wz_node(&self) -> Option<Self::Node>;

    fn get_name(&self) -> String;
    fn get_offset(&self) -> usize;
    fn get_block_size(&self) -> usize;
    fn get_full_path(&self) -> String;

    fn resolve_relative_path(&self, path: &str, force_parse: bool) -> Option<Self::Node>;

    
    fn update_parse_status(&self, status: bool);
    fn update_wz_file_meta(&self, wz_file_meta: WzFileMeta);
    fn update_wz_png_meta(&self, name: String, offset: usize, block_size: usize, property_type: WzPropertyType);

    fn transfer_childs(&self, to: &Self::Node);
    fn add_node_child(&self, child: Self::Node);
    fn add_node_childs(&self, childs: Vec<Self::Node>) {
        for child in childs {
            self.add_node_child(child);
        }
    }

    fn unparse_image(&self) -> Result<(), NodeParseError>;
    fn parse(&self) -> Result<(), NodeParseError>;
    fn parse_wz_image(&self) -> Result<(), NodeParseError>;
    fn parse_wz_directory(&self) -> Result<(), NodeParseError>;
    fn parse_wz_file(&self, patch_version: Option<i32>) -> Result<(), NodeParseError>;

    fn is_end(&self) -> bool;
    fn is_sound(&self) -> bool;
    fn is_string(&self) -> bool;
    fn is_png(&self) -> bool;
    fn is_lua(&self) -> bool;
    fn is_uol(&self) -> bool;

    fn get_string(&self) -> Result<String, WzStringParseError>;
    fn get_sound(&self) -> Result<Vec<u8>, WzSoundParseError>;
    fn get_image(&self) -> Result<DynamicImage, WzPngParseError>;
    fn save_sound(&self, path: &str, name: Option<&str>) -> Result<(), WzSoundParseError>;
    fn save_image(&self, path: &str, name: Option<&str>) -> Result<(), WzPngParseError>;
}