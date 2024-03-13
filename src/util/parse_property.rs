use crate::{property::{ WzPropertyType, Vector2D, WzPng, WzSoundMeta, get_sound_type_from_header }, NodeMethods, Reader, WzObjectType, WzSliceReader};
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

pub fn parse_property_list<Node: NodeMethods<Node = Node> + Clone>(parent: &Node, reader: &WzSliceReader, reader_offset: usize, origin_offset: usize) -> Result<(), WzPropertyParseError> {
    let parent = parent.clone();
    reader.seek(reader_offset);

    let entry_count = reader.read_wz_int()?;

    for _ in 0..entry_count {
        let name: String = reader.read_wz_string_block(origin_offset)?;
        let property_type: u8 = reader.read_u8()?;
        // let offset = origin_offset + reader.get_pos();

        match property_type {
            0 => {
                let node = Node::new_wz_primitive_property(&parent, WzPropertyType::Null, name);
                parent.add_node_child(node);
            },
            2 | 11 => {
                let num = reader.read_i16()?;
                let node = Node::new_wz_primitive_property(&parent, WzPropertyType::Short(num), name);
                parent.add_node_child(node);
            },
            3 | 19 => {
                let num = reader.read_wz_int()?;
                let node = Node::new_wz_primitive_property(&parent, WzPropertyType::Int(num), name);
                parent.add_node_child(node);
            },
            20 => {
                let num = reader.read_wz_int64()?;
                let node = Node::new_wz_primitive_property(&parent, WzPropertyType::Long(num), name);
                parent.add_node_child(node);
            },
            4 => {
                let float_type: u8 = reader.read_u8()?;
                match float_type {
                    0x80 => {
                        let num = reader.read_float()?;
                        let node = Node::new_wz_primitive_property(&parent, WzPropertyType::Float(num), name);
                        parent.add_node_child(node);
                    },
                    0 => {
                        let node = Node::new_wz_primitive_property(&parent, WzPropertyType::Float(0_f32), name);
                        parent.add_node_child(node);
                    },
                    _ => {

                    }
                }
            },
            5 => {
                let num = reader.read_double()?;
                let node = Node::new_wz_primitive_property(&parent, WzPropertyType::Double(num), name);
                parent.add_node_child(node);
            },
            8 => {
                let str_meta = reader.read_wz_string_block_meta(origin_offset)?;

                let node = Node::new_with_parent(
                    &parent,
                    WzObjectType::Property,
                    Some(WzPropertyType::String(str_meta.clone())),
                    name,
                    str_meta.offset,
                    str_meta.length as usize
                );
                parent.add_node_child(node);
            },
            9 => {
                let block_size = reader.read_u32()?;
                let next_pos = reader.get_pos() + block_size as usize;

                parse_extended_prop::<Node>(&parent, reader, next_pos, origin_offset, name)?;

                reader.seek(next_pos);
                
            },
            _ => {
                return Err(WzPropertyParseError::UnknownPropertyType(property_type, reader.get_pos()));
            }
        }
    }
    Ok(())
}

pub fn parse_extended_prop<Node: NodeMethods<Node = Node> + Clone>(parent: &Node, reader: &WzSliceReader, end_of_block: usize, origin_offset: usize, property_name: String) -> Result<(), WzPropertyParseError> {
    let extended_type = reader.read_u8()?;
    match extended_type {
        0x01 | crate::wz_image::WZ_IMAGE_HEADER_BYTE_WITH_OFFSET => {
            let name_offset = reader.read_i32()? as usize;
            parse_more::<Node>(parent, reader, end_of_block, origin_offset, property_name, reader.read_wz_string_at_offset(name_offset + origin_offset)?)?;
            Ok(())
        },
        0x00 | crate::wz_image::WZ_IMAGE_HEADER_BYTE_WITHOUT_OFFSET => {
            let _name = String::from("");
            parse_more::<Node>(parent, reader, end_of_block, origin_offset, property_name, String::from(""))?;
            Ok(())
        },
        _ => {
            Err(WzPropertyParseError::UnknownExtendedHeaderType(extended_type, reader.get_pos()))
        }
    }
}

pub fn parse_more<Node: NodeMethods<Node = Node> + Clone>(parent: &Node, reader: &WzSliceReader, end_of_block: usize, origin_offset: usize, property_name: String, extend_property_type: String) -> Result<(), WzPropertyParseError> {
    let extend_property_type = {
        if extend_property_type.is_empty() {
            reader.read_wz_string()?
        } else {
            extend_property_type
        }
    };

    match extend_property_type.as_str() {
        "Property" => {
            let node = Node::new_sub_property(parent, property_name, origin_offset, 0);

            if parse_property_list::<Node>(&node, reader, reader.get_pos() + 2, origin_offset).is_ok() {
                parent.add_node_child(node);
            }
        },
        "Canvas" => {
            reader.skip(1);
            let has_child = reader.read_u8().unwrap() == 1;

            let node = Node::new_with_parent(
                parent,
                WzObjectType::Property,
                None,
                String::new(),
                0,
                0
            );

            if has_child {
                reader.skip(2);
                parse_property_list::<Node>(&node, reader, reader.get_pos(), origin_offset)?;
            }

            let width = reader.read_wz_int()?;
            let height = reader.read_wz_int()?;
            let format1 = reader.read_wz_int()?;
            let format2 = reader.read_i8().unwrap();
            reader.skip(4);
            let canvas_slice_size = reader.read_i32().unwrap() - 1;
            reader.skip(1);
            let canvas_offset = reader.get_pos();
            let canvas_header = reader.read_u16().unwrap();
            let wz_png = WzPng::new(width as u32, height as u32, format1 as u32, format2 as u32, canvas_header as i32);

            node.update_wz_png_meta(property_name, canvas_offset, canvas_slice_size as usize, WzPropertyType::PNG(wz_png));

            parent.add_node_child(node);
        },
        "Shape2D#Convex2D" => {
            let node = Node::new_sub_property(parent, property_name.clone(), origin_offset, 0);
            let entry_count = reader.read_wz_int()?;
            for _ in 0..entry_count {
                parse_extended_prop::<Node>(&node, reader, end_of_block, origin_offset, property_name.clone())?;
            }
        },
        "Shape2D#Vector2D" => {
            let vec2 = Vector2D(
                reader.read_wz_int()?,
                reader.read_wz_int()?
            );
            let node = Node::new_wz_primitive_property(parent, WzPropertyType::Vector(vec2), property_name);
            parent.add_node_child(node);
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
            let sound_meta = WzSoundMeta::new(sound_offset, sound_size, header_offset, header_size, sound_duration, sound_type);

            let node = Node::new_with_parent(
                parent,
                WzObjectType::Property,
                Some(WzPropertyType::Sound(sound_meta)),
                property_name,
                sound_start_offset,
                end_of_block - sound_start_offset
            );

            parent.add_node_child(node);
        },
        "UOL" => {
            reader.skip(1);
            let str_meta = reader.read_wz_string_block_meta(origin_offset)?;
            let node = Node::new_with_parent(
                &parent,
                WzObjectType::Property,
                Some(WzPropertyType::UOL(str_meta.clone())),
                property_name,
                str_meta.offset,
                str_meta.length as usize
            );
            parent.add_node_child(node);
        },
        "RawData" => {
            reader.skip(1);
            let raw_data_size = reader.read_i32()?;
            let raw_data_offset = reader.get_pos();
            let node = Node::new_with_parent(
                parent,
                WzObjectType::Property,
                Some(WzPropertyType::RawData),
                property_name,
                raw_data_offset,
                raw_data_size as usize
            );
            parent.add_node_child(node);
        },
        _ => {
            return Err(WzPropertyParseError::UnknownExtendedPropertyType(extend_property_type.clone(), reader.get_pos()));
        }
    }

    Ok(())
}