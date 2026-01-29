use crate::{
    reader, util::version::PKGVersion, wz_image, Reader, WzHeader, WzImage, WzNode, WzNodeArc,
    WzNodeArcVec, WzNodeName, WzObjectType, WzReader, WzSliceReader,
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

#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum WzDirectoryType {
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

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) enum WzDirectoryVerifyStatus {
    #[default]
    Init,
    EntryCreated,
    Verified,
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
    #[cfg_attr(feature = "serde", serde(skip))]
    verify_status: WzDirectoryVerifyStatus,
    #[cfg_attr(feature = "serde", serde(skip))]
    entries: Vec<WzDirectoryEntry>,
}

impl WzDirectory {
    pub fn new(offset: usize, block_size: usize, reader: &Arc<WzReader>, is_parsed: bool) -> Self {
        Self {
            reader: reader.clone(),
            offset,
            block_size,
            hash: 0,
            is_parsed,
            verify_status: WzDirectoryVerifyStatus::Init,
            entries: Vec::new(),
        }
    }
    pub fn with_hash(mut self, hash: usize) -> Self {
        self.hash = hash;
        self
    }

    pub fn resolve_children(&mut self, parent: &WzNodeArc) -> Result<WzNodeArcVec, Error> {
        if self.block_size == 0 {
            return Ok(vec![]);
        }

        let reader = self.reader.create_slice_reader();

        reader.seek(self.offset);

        if self.verify_status == WzDirectoryVerifyStatus::Verified {
            return Ok(vec![]);
        } else if self.verify_status == WzDirectoryVerifyStatus::Init {
            if reader.header.ident == PKGVersion::V1 {
                self.entries = self.resolve_entry_pkg1(&reader)?;
            } else if reader.header.ident == PKGVersion::V2 {
                self.entries = self.resolve_entry_pkg2(&reader)?;
            } else {
                return Err(Error::UnknownPkgVersion);
            }

            self.verify_status = WzDirectoryVerifyStatus::EntryCreated;
        }

        let pkg2_hash1 = if reader.header.ident == PKGVersion::V2 {
            WzHeader::read_pkg2_hashes(reader.buf, reader.header.fstart)?[0]
        } else {
            0
        };

        let mut nodes: WzNodeArcVec = Vec::with_capacity(self.entries.len());

        // recalculate offset using current hash
        for entry in self.entries.iter_mut() {
            if reader.header.ident == PKGVersion::V1 {
                entry.offset = reader.read_wz_offset(
                    self.hash,
                    entry.encrypted_offset,
                    entry.calculation_offset,
                )?;
            } else if reader.header.ident == PKGVersion::V2 {
                entry.offset = reader.read_wz_offset_pkg2(
                    self.hash as u32,
                    pkg2_hash1,
                    entry.encrypted_offset,
                    entry.calculation_offset,
                )?;
            }

            if !reader.is_valid_pos(entry.offset + entry.size) {
                return Err(Error::InvalidWzVersion);
            }

            if entry.dir_type == WzDirectoryType::WzImage {
                let header_byte = reader.read_u8_at(entry.offset)?;
                if !wz_image::is_valid_image_header(header_byte) {
                    return Err(Error::InvalidWzVersion);
                }
                // should we try to parse the wz_image?
            } else if entry.dir_type == WzDirectoryType::WzDirectory {
                reader.seek(entry.offset);

                let entry_count = reader.read_wz_int()?;

                // entry count should not below 0
                if entry_count < 0 {
                    return Err(Error::InvalidWzVersion);
                }
            }

            let tuple = entry.into_wz_node_tuple(parent, self.hash, &self.reader);

            nodes.push(tuple);
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

        self.verify_status = WzDirectoryVerifyStatus::Verified;
        self.entries.clear();

        Ok(nodes)
    }

    fn resolve_entry_pkg1(&self, reader: &WzSliceReader) -> Result<Vec<WzDirectoryEntry>, Error> {
        let entry_count = reader.read_wz_int()?;

        if !(0..=1000000).contains(&entry_count) {
            return Err(Error::InvalidEntryCount);
        }

        let mut entries: Vec<WzDirectoryEntry> = Vec::with_capacity(entry_count as usize);

        for _ in 0..entry_count {
            let entry = WzDirectoryEntry::read_pkg1_entry(&reader)?;

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

            entries.push(entry);
        }

        Ok(entries)
    }

    fn resolve_entry_pkg2(&self, reader: &WzSliceReader) -> Result<Vec<WzDirectoryEntry>, Error> {
        let encrypted_entry_count = reader.read_wz_int()?;

        let mut wz_dir_entries: Vec<WzDirectoryEntry> = vec![];

        // currently we don't know how to decrypt the entry_count, so we will just keep reading until get the encrypted_offset_count
        loop {
            let entry = WzDirectoryEntry::read_pkg2_entry(&reader, encrypted_entry_count)?;

            match entry.dir_type {
                // im using the UnknownType to indicate the end of the entries
                WzDirectoryType::UnknownType => {
                    break;
                }
                WzDirectoryType::NewUnknownType(dir_byte) => {
                    return Err(Error::UnknownWzDirectoryType(dir_byte, reader.pos.get()));
                }
                _ => {
                    wz_dir_entries.push(entry);
                }
            }
        }

        let encrypted_offset_count = reader.read_wz_int()?;

        if encrypted_offset_count != encrypted_entry_count || wz_dir_entries.len() == 0 {
            return Err(Error::InvalidWzVersion);
        }

        for entry in wz_dir_entries.iter_mut() {
            // different from pkg1, use the offset 'after' read the encrypted offset
            entry.encrypted_offset = reader.read_u32()?;
            entry.calculation_offset = reader.pos.get();
        }

        Ok(wz_dir_entries)
    }
}

#[derive(Debug, Default, Clone)]
struct WzDirectoryEntry {
    name: WzNodeName,
    dir_type: WzDirectoryType,
    size: usize,
    calculation_offset: usize,
    encrypted_offset: u32,
    offset: usize,
    _checksum: i32,
}

impl WzDirectoryEntry {
    pub fn read_pkg1_entry(reader: &WzSliceReader) -> Result<Self, Error> {
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
        entry.calculation_offset = reader.pos.get();
        entry.encrypted_offset = reader.read_u32()?;

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

        Ok(entry)
    }

    pub fn into_wz_node_tuple(
        &self,
        parent: &WzNodeArc,
        hash: usize,
        reader: &Arc<WzReader>,
    ) -> (WzNodeName, WzNodeArc) {
        let node: WzNode;

        match self.dir_type {
            WzDirectoryType::WzDirectory => {
                let wz_dir =
                    WzDirectory::new(self.offset, self.size, reader, false).with_hash(hash);
                node = WzNode::new(&self.name, wz_dir, Some(parent));
            }
            WzDirectoryType::WzImage => {
                let wz_image = WzImage::new(&self.name, self.offset, self.size, reader);
                node = WzNode::new(&self.name, wz_image, Some(parent));
            }
            _ => {
                node = WzNode::empty();
            }
        }

        (self.name.clone(), node.into_lock())
    }
}
