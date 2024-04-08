use std::sync::Arc;
use crate::{property::{ get_sound_type_from_header, Vector2D }, Reader, WzNodeLink, WzNodeLinkArc, WzNodeLinkArcVec, WzObjectType, WzReader, WzSliceReader};
use crate::property::{WzSubProperty, WzValue, WzString, WzSound, WzPng, WzRawData};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WzPropertyParseError {
    #[error("Unknown property type: {0}, at position: {1}")]
    UnknownPropertyType(u8, usize),

    #[error("Unknown extended header type: {0}, at position: {1}")]
    UnknownExtendedHeaderType(u8, usize),

    #[error("Unknown extended property type: {0}, at position: {1}")]
    UnknownExtendedPropertyType(String, usize),

    #[error("Binary reading error")]
    ReaderError(#[from] scroll::Error),
}

pub fn parse_property_list(parent: &WzNodeLinkArc, org_reader: &Arc<WzReader> ,reader: &WzSliceReader, reader_offset: usize, origin_offset: usize) -> Result<WzNodeLinkArcVec, WzPropertyParseError> {
    // let parent = parent.clone();
    reader.seek(reader_offset);

    let entry_count = reader.read_wz_int()?;

    let mut childs: Vec<(String, WzNodeLinkArc)> = Vec::with_capacity(entry_count as usize);
    for _ in 0..entry_count {
        let name: String = reader.read_wz_string_block(origin_offset)?;
        let property_type: u8 = reader.read_u8()?;
        // let offset = origin_offset + reader.get_pos();

        match property_type {
            0 => {
                let node = WzNodeLink::new(name.clone(), WzObjectType::Value(WzValue::Null), Some(parent));
                childs.push((name, node.into_lock()));
            },
            2 | 11 => {
                let num = reader.read_i16()?;
                let node = WzNodeLink::new(name.clone(), WzObjectType::Value(WzValue::Short(num)), Some(parent));
                childs.push((name, node.into_lock()));
            },
            3 | 19 => {
                let num = reader.read_wz_int()?;
                let node = WzNodeLink::new(name.clone(), WzObjectType::Value(WzValue::Int(num)), Some(parent));
                childs.push((name, node.into_lock()));
            },
            20 => {
                let num = reader.read_wz_int64()?;
                let node = WzNodeLink::new(name.clone(), WzObjectType::Value(WzValue::Long(num)), Some(parent));
                childs.push((name, node.into_lock()));
            },
            4 => {
                let float_type: u8 = reader.read_u8()?;
                match float_type {
                    0x80 => {
                        let num = reader.read_float()?;
                        let node = WzNodeLink::new(name.clone(), WzObjectType::Value(WzValue::Float(num)), Some(parent));
                        childs.push((name, node.into_lock()));
                    },
                    _ => {
                        let node = WzNodeLink::new(name.clone(), WzObjectType::Value(WzValue::Float(0_f32)), Some(parent));
                        childs.push((name, node.into_lock()));
                    }
                }
            },
            5 => {
                let num = reader.read_double()?;
                let node = WzNodeLink::new(name.clone(), WzObjectType::Value(WzValue::Double(num)), Some(parent));
                childs.push((name, node.into_lock()));
            },
            8 => {
                let str_meta = reader.read_wz_string_block_meta(origin_offset)?;
                let node = WzNodeLink::new(
                    name.clone(),
                    WzObjectType::Value(WzValue::String(WzString::from_meta(str_meta, org_reader))),
                    Some(parent)
                );
                childs.push((name, node.into_lock()));
            },
            9 => {
                let block_size = reader.read_u32()?;
                let next_pos = reader.get_pos() + block_size as usize;

                let node = parse_extended_prop(&parent, org_reader, reader, next_pos, origin_offset, name)?;

                childs.push(node);

                reader.seek(next_pos);
                
            },
            _ => {
                return Err(WzPropertyParseError::UnknownPropertyType(property_type, reader.get_pos()));
            }
        }
    }
    Ok(childs)
}

pub fn parse_extended_prop(parent: &WzNodeLinkArc, org_reader: &Arc<WzReader>, reader: &WzSliceReader, end_of_block: usize, origin_offset: usize, property_name: String) -> Result<(String, WzNodeLinkArc), WzPropertyParseError> {
    let extended_type = reader.read_u8()?;
    match extended_type {
        0x01 | crate::wz_image::WZ_IMAGE_HEADER_BYTE_WITH_OFFSET => {
            let name_offset = reader.read_i32()? as usize;
            parse_more(parent, org_reader, reader, end_of_block, origin_offset, property_name, reader.read_wz_string_at_offset(name_offset + origin_offset)?)
        },
        0x00 | crate::wz_image::WZ_IMAGE_HEADER_BYTE_WITHOUT_OFFSET => {
            let _name = String::from("");
            parse_more(parent, org_reader, reader, end_of_block, origin_offset, property_name, String::from(""))
        },
        _ => {
            Err(WzPropertyParseError::UnknownExtendedHeaderType(extended_type, reader.get_pos()))
        }
    }
}

pub fn parse_more(parent: &WzNodeLinkArc, org_reader: &Arc<WzReader>, reader: &WzSliceReader, end_of_block: usize, origin_offset: usize, property_name: String, extend_property_type: String) -> Result<(String, WzNodeLinkArc), WzPropertyParseError> {
    let extend_property_type = {
        if extend_property_type.is_empty() {
            reader.read_wz_string()?
        } else {
            extend_property_type
        }
    };

    match extend_property_type.as_str() {
        "Property" => {
            let node = WzNodeLink::new(
                property_name.clone(),
                WzObjectType::Property(WzSubProperty::Property),
                Some(parent)
            ).into_lock();

            let childs = parse_property_list(&node, org_reader, reader, reader.get_pos() + 2, origin_offset)?;
            
            {
                let mut node_write = node.write().unwrap();
                for (name, child) in childs {
                    node_write.children.insert(name, child);
                }
            }

            Ok((property_name, node))
        },
        "Canvas" => {
            reader.skip(1);
            let has_child = reader.read_u8()? == 1;

            let node = WzNodeLink::new(
                property_name.clone(),
                WzObjectType::Property(WzSubProperty::Property),
                Some(parent)
            ).into_lock();

            if has_child {
                reader.skip(2);
                let childs = parse_property_list(&node, org_reader, reader, reader.get_pos(), origin_offset)?;
                let mut node_write = node.write().unwrap();
                for (name, child) in childs {
                    node_write.children.insert(name, child);
                }
            }

            let width = reader.read_wz_int()?;
            let height = reader.read_wz_int()?;
            let format1 = reader.read_wz_int()?;
            let format2 = reader.read_i8()?;
            reader.skip(4);
            let canvas_slice_size = (reader.read_i32()? - 1) as usize;
            reader.skip(1);
            let canvas_offset = reader.get_pos();
            let canvas_header = reader.read_u16()?;
            let wz_png = WzPng::new(org_reader, (width as u32, height as u32), (format1 as u32, format2 as u32), (canvas_offset, canvas_slice_size), canvas_header as i32);

            if let Ok(mut node) = node.write() {
                node.object_type = WzObjectType::Property(WzSubProperty::PNG(Box::new(wz_png)));
            }

            Ok((property_name, node))
        },
        "Shape2D#Convex2D" => {
            let node = WzNodeLink::new(
                property_name.clone(),
                WzObjectType::Property(WzSubProperty::Convex),
                Some(parent)
            ).into_lock();

            let entry_count = reader.read_wz_int()?;

            {
                let mut node_write = node.write().unwrap();
                for _ in 0..entry_count {
                    let parsed_node = parse_extended_prop(&node, org_reader, reader, end_of_block, origin_offset, property_name.clone())?;
    
                    node_write.children.insert(parsed_node.0, parsed_node.1);
                }
            }

            Ok((property_name, node))
        },
        "Shape2D#Vector2D" => {
            let vec2 = Vector2D(
                reader.read_wz_int()?,
                reader.read_wz_int()?
            );
            let node = WzNodeLink::new(property_name.clone(), WzObjectType::Value(WzValue::Vector(vec2)), Some(parent));

            Ok((property_name, node.into_lock()))
        },
        "Sound_DX8" => {
            reader.skip(1);
            let sound_start_offset = reader.get_pos();
            let sound_size = reader.read_wz_int()? as u32;
            let sound_duration = reader.read_wz_int()? as u32;
            let sound_offset = end_of_block - (sound_size as usize);
            
            let header_offset: usize = reader.get_pos();

            
            let header_size = sound_offset - header_offset;

            let sound_type = get_sound_type_from_header(&reader.buf[header_offset..header_offset+header_size], sound_size, sound_duration);
            let sound = WzSound::new(org_reader, sound_offset, sound_size, header_offset, header_size, sound_duration, sound_type);

            let node = WzNodeLink::new(
                property_name.clone(),
                WzObjectType::Property(WzSubProperty::Sound(Box::new(sound))),
                Some(parent)
            );

            Ok((property_name, node.into_lock()))
        },
        "UOL" => {
            reader.skip(1);
            let str_meta = reader.read_wz_string_block_meta(origin_offset)?;
            let node = WzNodeLink::new(
                property_name.clone(),
                WzObjectType::Value(WzValue::UOL(WzString::from_meta(str_meta, org_reader))),
                Some(parent)
            );

            Ok((property_name, node.into_lock()))
        },
        "RawData" => {
            reader.skip(1);
            let raw_data_size = reader.read_i32()? as usize;
            let raw_data_offset = reader.get_pos();
            let node = WzNodeLink::new(
                property_name.clone(),
                WzObjectType::Value(WzValue::RawData(WzRawData::new(org_reader, raw_data_offset, raw_data_size))),
                Some(parent)
            );

            Ok((property_name, node.into_lock()))
        },
        _ => {
            Err(WzPropertyParseError::UnknownExtendedPropertyType(extend_property_type.clone(), reader.get_pos()))
        }
    }
}