use wz_reader::property::Vector2D;
use wz_reader::{WzNode, WzNodeArc, WzObjectType, property::WzValue};
use serde_json::{ Map, Value };

fn walk_node_and_to_json(node_arc: &WzNodeArc, json: &mut Map<String, Value>) {
    {
        node_arc.write().unwrap().parse(node_arc).unwrap();
    }
    let node = node_arc.read().unwrap();
    match &node.object_type {
        WzObjectType::Value(value_type) => {
            match value_type {
                WzValue::Int(value) => {
                    json.insert(node.name.clone(), Value::Number(serde_json::Number::from(*value)));
                },
                WzValue::Short(value) => {
                    json.insert(node.name.clone(), Value::Number(serde_json::Number::from(*value)));
                },
                WzValue::Long(value) => {
                    json.insert(node.name.clone(), Value::Number(serde_json::Number::from(*value)));
                },
                WzValue::Float(value) => {
                    json.insert(node.name.clone(), Value::Number(serde_json::Number::from_f64((*value).into()).unwrap()));
                },
                WzValue::Double(value) => {
                    json.insert(node.name.clone(), Value::Number(serde_json::Number::from_f64(*value).unwrap()));
                },
                WzValue::String(wz_string) | WzValue::UOL(wz_string) => {
                    let string = wz_string.get_string();
                    match string {
                        Ok(string) => {
                            json.insert(node.name.clone(), Value::String(string));
                        },
                        Err(_) => {
                            json.insert(node.name.clone(), Value::String(String::from("")));
                        }
                    }
                },
                WzValue::Vector(Vector2D (x, y)) => {
                    let mut vec = Map::new();
                    vec.insert("x".to_string(), Value::Number(serde_json::Number::from(*x)));
                    vec.insert("y".to_string(), Value::Number(serde_json::Number::from(*y)));
                    json.insert(node.name.clone(), Value::Object(vec));
                },
                WzValue::Null | WzValue::RawData(_) | WzValue::Lua(_) => {
                    json.insert(node.name.clone(), Value::Null);
                },
            }
        },
        WzObjectType::Directory(_) | WzObjectType::Image(_) | WzObjectType::File(_) | WzObjectType::Property(_) => {
            let mut child_json = Map::new();
            if node.children.len() != 0 {
                for value in node.children.values() {
                    walk_node_and_to_json(value, &mut child_json);
                }
                json.insert(node.name.clone(), Value::Object(child_json));
            }
        }
    }
}

fn main() {
    /* resolve single wz file */
    let node: WzNodeArc = WzNode::from_wz_file(r"D:\MapleStory\Data\UI\UI_000.wz", None).unwrap().into();

    let mut node_write = node.write().unwrap();

    node_write.parse(&node).unwrap();

    let mut json = Map::new();

    for value in node_write.children.values() {
        walk_node_and_to_json(value, &mut json);
    }

    let json_string = serde_json::to_string_pretty(&Value::Object(json)).unwrap();

    std::fs::write("UI_000.json", json_string).unwrap();
}