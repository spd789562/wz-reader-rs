use wz_reader::property::Vector2D;
use wz_reader::{NodeMethods, WzObjectType, property::WzPropertyType};
use wz_reader::arc::WzNodeArc;
use serde_json::{ Map, Value };

fn walk_node_and_to_json(node_arc: WzNodeArc, json: &mut Map<String, Value>) {
    node_arc.parse().unwrap();
    let node = node_arc.read().unwrap();
    match &node.object_type {
        WzObjectType::Property => {
            match &node.property_type {
                WzPropertyType::Int(value) => {
                    json.insert(node.name.clone(), Value::Number(serde_json::Number::from(*value)));
                },
                WzPropertyType::Short(value) => {
                    json.insert(node.name.clone(), Value::Number(serde_json::Number::from(*value)));
                },
                WzPropertyType::Long(value) => {
                    json.insert(node.name.clone(), Value::Number(serde_json::Number::from(*value)));
                },
                WzPropertyType::Float(value) => {
                    json.insert(node.name.clone(), Value::Number(serde_json::Number::from_f64((*value).into()).unwrap()));
                },
                WzPropertyType::Double(value) => {
                    json.insert(node.name.clone(), Value::Number(serde_json::Number::from_f64(*value).unwrap()));
                },
                WzPropertyType::String(_) | WzPropertyType::UOL(_) => {
                    let string = node_arc.get_string();
                    match string {
                        Ok(string) => {
                            json.insert(node.name.clone(), Value::String(string));
                        },
                        Err(_) => {
                            json.insert(node.name.clone(), Value::String(String::from("")));
                        }
                    }
                },
                WzPropertyType::Vector(Vector2D (x, y)) => {
                    let mut vec = Map::new();
                    vec.insert("x".to_string(), Value::Number(serde_json::Number::from(*x)));
                    vec.insert("y".to_string(), Value::Number(serde_json::Number::from(*y)));
                    json.insert(node.name.clone(), Value::Object(vec));
                },
                WzPropertyType::SubProperty | WzPropertyType::Convex | WzPropertyType::PNG(_) => {
                    let mut child_json = Map::new();
                    for value in node.children.values() {
                        walk_node_and_to_json(value.clone(), &mut child_json);
                    }
                    json.insert(node.name.clone(), Value::Object(child_json));
                },
                WzPropertyType::Null | WzPropertyType::RawData | WzPropertyType::Sound(_) | WzPropertyType::Lua => {
                    json.insert(node.name.clone(), Value::Null);
                },
            }
        },
        WzObjectType::Directory | WzObjectType::Image | WzObjectType::File => {
            let mut child_json = Map::new();
            for value in node.children.values() {
                walk_node_and_to_json(value.clone(), &mut child_json);
            }
            json.insert(node.name.clone(), Value::Object(child_json));
        }
        _ => {}
    }
}

fn main() {
    /* resolve single wz file */
    let node = WzNodeArc::new_wz_file(r"D:\MapleStory\Data\UI\UI_000.wz", None);

    node.parse().unwrap();

    let mut json = Map::new();

    for (key, value) in node.read().unwrap().children.iter() {
        let mut child_json = Map::new();
        walk_node_and_to_json(value.clone(), &mut child_json);
        json.insert(key.clone(), Value::Object(child_json));
    }

    let json_string = serde_json::to_string_pretty(&Value::Object(json)).unwrap();

    std::fs::write("UI_000.json", json_string).unwrap();
}