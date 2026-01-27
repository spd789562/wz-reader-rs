use crate::{
    reader, PKGVersion, Reader, WzHeader, WzImage, WzNode, WzNodeArc, WzNodeArcVec, WzNodeName,
    WzObjectType, WzReader, WzSliceReader,
};
use std::sync::Arc;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Lua parse error")]
    LuaParseError,
    #[error("parse as wz image failed, pos at {0}")]
    ParseError(usize),
    #[error("New Wz image header found. b = {0}, offset = {1}")]
    UnknownWzDirectoryType(u8, usize),
    #[error("Invalid wz version used for decryption, try parsing other version numbers.")]
    InvalidWzVersion,
    #[error("Entry count overflow, Invalid wz version used for decryption, try parsing other version numbers.")]
    InvalidEntryCount,
    #[error("Unknown pkg version, can't resolve children")]
    UnknownPkgVersion,
    #[error("Binary reading error")]
    ReaderError(#[from] reader::Error),
}

#[derive(Debug, Default)]
#[repr(u8)]
enum WzDirectoryType {
    #[default]
    UnknownType = 1,
    /// directory type and name maybe at some where alse, but usually is WzDirectory
    MetaAtOffset = 2,
    WzDirectory = 3,
    WzImage = 4,
    NewUnknownType(u8),
}

impl From<u8> for WzDirectoryType {
    fn from(value: u8) -> Self {
        match value {
            1 => WzDirectoryType::UnknownType,
            2 => WzDirectoryType::MetaAtOffset,
            3 => WzDirectoryType::WzDirectory,
            4 => WzDirectoryType::WzImage,
            _ => WzDirectoryType::NewUnknownType(value),
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct WzDirectory {
    #[cfg_attr(feature = "serde", serde(skip))]
    pub reader: Arc<WzReader>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub offset: usize,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub block_size: usize,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub hash: usize,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub is_parsed: bool,
}

impl WzDirectory {
    pub fn new(offset: usize, block_size: usize, reader: &Arc<WzReader>, is_parsed: bool) -> Self {
        Self {
            reader: reader.clone(),
            offset,
            block_size,
            hash: 0,
            is_parsed,
        }
    }
    pub fn with_hash(mut self, hash: usize) -> Self {
        self.hash = hash;
        self
    }

    pub fn verify_hash(&self) -> Result<(), Error> {
        let reader = self.reader.create_slice_reader();

        reader.seek(self.offset);

        let entry_count = reader.read_wz_int()?;

        if !(0..=1000000).contains(&entry_count) {
            return Err(Error::InvalidEntryCount);
        }

        for _ in 0..entry_count {
            let entry = WzDirectoryEntry::read_pkg1_entry(&reader, self.hash)?;

            match entry.dir_type {
                WzDirectoryType::UnknownType => {
                    /* unknown, just skip this chunk, probably checksum(2), file size(4) and hash(4)*/
                    reader.skip(4 + 4 + 2);
                    continue;
                }
                WzDirectoryType::NewUnknownType(dir_byte) => {
                    return Err(Error::UnknownWzDirectoryType(dir_byte, reader.pos.get()));
                }
                _ => {
                    // do nothing
                }
            }

            if !reader.is_valid_pos(entry.offset + entry.size) {
                return Err(Error::InvalidWzVersion);
            }
        }

        Ok(())
    }

    pub fn resolve_children(&self, parent: &WzNodeArc) -> Result<WzNodeArcVec, Error> {
        let reader = self.reader.create_slice_reader();

        reader.seek(self.offset);

        if reader.header.ident == PKGVersion::V1 {
            self.resolve_children_pkg1(&reader, parent)
        } else if reader.header.ident == PKGVersion::V2 {
            self.resolve_children_pkg2(&reader, parent)
        } else {
            Err(Error::UnknownPkgVersion)
        }
    }

    fn resolve_children_pkg1(
        &self,
        reader: &WzSliceReader,
        parent: &WzNodeArc,
    ) -> Result<WzNodeArcVec, Error> {
        let entry_count = reader.read_wz_int()?;

        if !(0..=1000000).contains(&entry_count) {
            return Err(Error::InvalidEntryCount);
        }

        let mut nodes: WzNodeArcVec = Vec::with_capacity(entry_count as usize);

        for _ in 0..entry_count {
            let entry = WzDirectoryEntry::read_pkg1_entry(&reader, self.hash)?;

            match entry.dir_type {
                WzDirectoryType::UnknownType => {
                    /* unknown, just skip this chunk, probably checksum(2), file size(4) and hash(4)*/
                    reader.skip(4 + 4 + 2);
                    continue;
                }
                WzDirectoryType::NewUnknownType(dir_byte) => {
                    println!("NewUnknownType: {}", dir_byte);
                    return Err(Error::UnknownWzDirectoryType(dir_byte, reader.pos.get()));
                }
                _ => {
                    // do nothing
                }
            }

            if !reader.is_valid_pos(entry.offset + entry.size) {
                return Err(Error::InvalidWzVersion);
            }

            nodes.push(entry.into_wz_node_tuple(parent, &self));
        }

        // if there has any directory, parse it since it's probably cheep
        for (_, node) in nodes.iter() {
            let mut write = node.write().unwrap();
            if let WzObjectType::Directory(dir) = &mut write.object_type {
                let children = dir.resolve_children(node)?;

                for (name, child) in children {
                    write.children.insert(name, child);
                }
            }
        }

        Ok(nodes)
    }

    fn resolve_children_pkg2(
        &self,
        reader: &WzSliceReader,
        parent: &WzNodeArc,
    ) -> Result<WzNodeArcVec, Error> {
        let encrypted_entry_count = reader.read_wz_int()?;

        let wz_dir_entries: Vec<WzDirectoryEntry> = vec![];

        loop {
            let entry = WzDirectoryEntry::read_pkg2_entry(&reader, encrypted_entry_count)?;

            match entry.dir_type {
                WzDirectoryType::UnknownType => {
                    break;
                }
                WzDirectoryType::NewUnknownType(dir_byte) => {
                    println!("NewUnknownType: {}", dir_byte);
                    return Err(Error::UnknownWzDirectoryType(dir_byte, reader.pos.get()));
                }
                _ => {
                    // do nothing
                }
            }
        }

        let encrypted_offset_count = reader.read_wz_int()?;

        if encrypted_offset_count != encrypted_entry_count || wz_dir_entries.len() == 0 {
            return Err(Error::InvalidWzVersion);
        }

        let mut nodes: WzNodeArcVec = Vec::with_capacity(wz_dir_entries.len() as usize);

        let [hash1, _] = WzHeader::read_pkg2_hashes(reader.buf, reader.header.fstart)?;

        for mut entry in wz_dir_entries {
            entry.offset = reader.read_wz_offset_pkg2(self.hash, hash1 as usize, None)?;

            nodes.push(entry.into_wz_node_tuple(parent, &self));
        }

        Ok(nodes)
    }
}

#[derive(Debug, Default)]
struct WzDirectoryEntry {
    name: WzNodeName,
    dir_type: WzDirectoryType,
    size: usize,
    offset: usize,
    _checksum: i32,
}

impl WzDirectoryEntry {
    pub fn read_pkg1_entry(reader: &WzSliceReader, hash: usize) -> Result<Self, Error> {
        let mut entry = WzDirectoryEntry::default();
        entry.dir_type = reader.read_u8()?.into();

        match entry.dir_type {
            WzDirectoryType::UnknownType => {
                return Ok(entry);
            }
            WzDirectoryType::MetaAtOffset => {
                let str_offset = reader.read_i32()?;

                let offset = reader.header.fstart + str_offset as usize;

                entry.dir_type = reader.read_u8_at(offset)?.into();
                entry.name = reader.read_wz_string_at_offset(offset + 1)?.into();
            }
            WzDirectoryType::WzDirectory | WzDirectoryType::WzImage => {
                entry.name = reader.read_wz_string()?.into();
            }
            WzDirectoryType::NewUnknownType(_) => {
                return Ok(entry);
            }
        }

        entry.size = reader.read_wz_int()? as usize;
        entry._checksum = reader.read_wz_int()?;
        entry.offset = reader.read_wz_offset(hash, None)?;

        Ok(entry)
    }

    pub fn read_pkg2_entry(
        reader: &WzSliceReader,
        encrypted_entry_count: i32,
    ) -> Result<Self, Error> {
        let mut entry = WzDirectoryEntry::default();
        entry.dir_type = reader.read_u8()?.into();

        match entry.dir_type {
            WzDirectoryType::WzDirectory | WzDirectoryType::WzImage => {
                entry.name = reader.read_wz_string()?.into();
            }
            WzDirectoryType::NewUnknownType(_) => {
                let current_pos = reader.pos.get();
                reader.pos.set(current_pos - 1);
                let test_value = reader.read_wz_int()?;
                // if reach the value is same as encrypted_entry_count, mean we reach the end of the entries
                if test_value == encrypted_entry_count {
                    reader.pos.set(current_pos - 1);
                    entry.dir_type = WzDirectoryType::UnknownType;
                }
                return Ok(entry);
            }
            _ => {
                return Err(Error::InvalidWzVersion);
            }
        }

        entry.size = reader.read_wz_int()? as usize;
        entry._checksum = reader.read_wz_int()?;
        entry.offset = reader.pos.get();

        Ok(entry)
    }

    pub fn into_wz_node_tuple(
        self,
        parent: &WzNodeArc,
        wz_dir: &WzDirectory,
    ) -> (WzNodeName, WzNodeArc) {
        let node: WzNode;

        match self.dir_type {
            WzDirectoryType::WzDirectory => {
                let wz_dir = WzDirectory::new(self.offset, self.size, &wz_dir.reader, false)
                    .with_hash(wz_dir.hash);
                node = WzNode::new(&self.name, wz_dir, Some(parent));
            }
            WzDirectoryType::WzImage => {
                let wz_image = WzImage::new(&self.name, self.offset, self.size, &wz_dir.reader);
                node = WzNode::new(&self.name, wz_image, Some(parent));
            }
            _ => {
                node = WzNode::empty();
            }
        }

        (self.name, node.into_lock())
    }
}
