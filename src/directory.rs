use crate::{
    property::WzStringMeta,
    reader,
    util::{
        offset::{self, WzOffsetMeta},
        profile::WzProfile,
        version::PKGVersion,
    },
    wz_image, Reader, WzHeader, WzImage, WzNode, WzNodeArc, WzNodeArcVec, WzNodeName, WzObjectType,
    WzReader, WzSliceReader,
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
    pub profile: WzProfile,
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
            profile: WzProfile::default(),
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

        self.prepare_entries()?;

        if !self.calculate_offset_and_verify().is_ok() {
            return Err(Error::InvalidWzVersion);
        }

        let nodes: WzNodeArcVec = self
            .entries
            .iter()
            .map(|entry| entry.into_wz_node_tuple(parent, self.hash, Arc::clone(&self.reader)))
            .collect();

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
            let entry = WzDirectoryEntry::read_pkg2_entry(
                &reader,
                encrypted_entry_count,
                wz_dir_entries.len() == 0,
            )?;

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
            entry.calculation_offset = reader.pos.get();
            entry.encrypted_offset = reader.read_u32()?;
        }

        Ok(wz_dir_entries)
    }

    pub fn prepare_entries(&mut self) -> Result<(), Error> {
        if self.verify_status != WzDirectoryVerifyStatus::Init {
            return Ok(());
        }

        let reader = self.reader.create_slice_reader();

        reader.seek(self.offset);

        if reader.header.ident == PKGVersion::V1 {
            self.entries = self.resolve_entry_pkg1(&reader)?;
        } else if reader.header.ident == PKGVersion::V2 {
            self.entries = self.resolve_entry_pkg2(&reader)?;
        } else {
            return Err(Error::UnknownPkgVersion);
        }

        self.verify_status = WzDirectoryVerifyStatus::EntryCreated;

        Ok(())
    }

    pub fn calculate_offset_and_verify(&mut self) -> Result<(), Error> {
        let reader = self.reader.create_slice_reader();

        let pkg2_hash1 = if reader.header.ident == PKGVersion::V2 {
            WzHeader::read_pkg2_hashes(reader.buf, reader.header.fstart)?[0]
        } else {
            0
        };

        let offset_calculator = self.profile.offset_version.get_calculator();

        for entry in self.entries.iter_mut() {
            let meta = WzOffsetMeta {
                hash: self.hash as u32,
                encrypted_offset: entry.encrypted_offset,
                offset: entry.calculation_offset,
                pkg2_hash1: pkg2_hash1,
            };

            entry.offset = offset_calculator(&reader.header, &meta)?;

            if entry.verify(&reader).is_err() {
                return Err(Error::InvalidWzVersion);
            }
        }

        Ok(())
    }

    pub fn verify_string_decryptor(&mut self) -> bool {
        if self.verify_status != WzDirectoryVerifyStatus::EntryCreated {
            return false;
        }
        if self.entries.len() == 0 {
            return true;
        }
        self.entries[0].resolve_name(&self.reader).is_ok()
    }
}

#[derive(Debug, Default, Clone)]
struct WzDirectoryEntry {
    name: WzStringMeta,
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
                entry.name = reader.read_wz_string_meta_at(offset + 1)?;
            }
            WzDirectoryType::WzDirectory | WzDirectoryType::WzImage => {
                entry.name = reader.read_wz_string_meta()?;
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
        use_pkg2_dir_read: bool,
    ) -> Result<Self, Error> {
        let mut entry = WzDirectoryEntry::default();
        entry.dir_type = reader.read_u8()?.into();

        match entry.dir_type {
            WzDirectoryType::WzDirectory | WzDirectoryType::WzImage => {
                // currently only the first name is using read_wz_string_pkg2_dir
                if use_pkg2_dir_read && reader.pkg2_keys.read().unwrap().is_pkg2() {
                    entry.name = reader.read_wz_string_pkg2_dir_meta()?;
                } else {
                    entry.name = reader.read_wz_string_meta()?;
                }
            }
            WzDirectoryType::NewUnknownType(_) => {
                let current_pos = reader.pos.get();
                reader.pos.set(current_pos - 1);
                let test_wz_int = reader.read_wz_int()?;

                // if reach the value is same as encrypted_entry_count, mean we reach the end of the entries
                if test_wz_int == encrypted_entry_count {
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

    pub fn verify(&self, reader: &WzSliceReader) -> Result<(), Error> {
        if !reader.is_valid_pos(self.offset + self.size) {
            return Err(Error::InvalidWzVersion);
        }

        if self.dir_type == WzDirectoryType::WzImage {
            let header_byte = reader.read_u8_at(self.offset)?;
            if !wz_image::is_valid_image_header(header_byte) {
                return Err(Error::InvalidWzVersion);
            }
            // should we try to parse the wz_image?
        } else if self.dir_type == WzDirectoryType::WzDirectory {
            reader.seek(self.offset);

            let entry_count = reader.read_wz_int()?;

            // entry count should not below 0
            if entry_count < 0 {
                return Err(Error::InvalidWzVersion);
            }
        }
        Ok(())
    }

    pub fn resolve_name(&self, reader: &Arc<WzReader>) -> Result<WzNodeName, Error> {
        Ok(reader
            .resolve_wz_string_meta(
                &self.name.string_type,
                self.name.offset,
                self.name.length as usize,
            )?
            .into())
    }

    pub fn try_into_wz_node_tuple(
        &self,
        parent: &WzNodeArc,
        hash: usize,
        reader: Arc<WzReader>,
    ) -> Result<(WzNodeName, WzNodeArc), Error> {
        let node: WzNode;
        let name: WzNodeName = self.resolve_name(&reader)?;

        match self.dir_type {
            WzDirectoryType::WzDirectory => {
                let wz_dir =
                    WzDirectory::new(self.offset, self.size, &reader, false).with_hash(hash);
                node = WzNode::new(&name, wz_dir, Some(parent));
            }
            WzDirectoryType::WzImage => {
                let wz_image = WzImage::new(&name, self.offset, self.size, &reader);
                node = WzNode::new(&name, wz_image, Some(parent));
            }
            _ => {
                node = WzNode::empty();
            }
        }

        Ok((name.clone(), node.into_lock()))
    }

    pub fn into_wz_node_tuple(
        &self,
        parent: &WzNodeArc,
        hash: usize,
        reader: Arc<WzReader>,
    ) -> (WzNodeName, WzNodeArc) {
        self.try_into_wz_node_tuple(parent, hash, reader).unwrap()
    }
}
