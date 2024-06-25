use serde_json::{Map, Value};
use wz_reader::{util::node_util, WzNode, WzNodeArc, WzObjectType};

fn walk_node_and_to_json(node_arc: &WzNodeArc, json: &mut Map<String, Value>) {
    node_util::parse_node(node_arc).unwrap();
    let node = node_arc.read().unwrap();
    match &node.object_type {
        WzObjectType::Value(value_type) => {
            json.insert(node.name.to_string(), value_type.clone().into());
        }
        WzObjectType::Directory(_)
        | WzObjectType::Image(_)
        | WzObjectType::File(_)
        | WzObjectType::Property(_) => {
            let mut child_json = Map::new();
            if node.children.len() != 0 {
                for value in node.children.values() {
                    walk_node_and_to_json(value, &mut child_json);
                }
                json.insert(node.name.to_string(), Value::Object(child_json));
            }
        }
    }
}

// usage:
//   cargo run --example wz_to_json -- "path/to/some.wz" "ouput/path"
//   cargo run --example wz_to_json -- "D:\Path\To\some.wz" ".\output"
fn main() {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().expect("Need path to wz file as 1st arg");
    let out = args.next().expect("Need out json dir as 2nd arg");

    /* resolve single wz file */
    let node: WzNodeArc = WzNode::from_wz_file(&path, None).unwrap().into();

    let mut node_write = node.write().unwrap();

    let file_name = node_write.name.to_string();

    node_write.parse(&node).unwrap();

    let mut json = Map::new();

    for value in node_write.children.values() {
        walk_node_and_to_json(value, &mut json);
    }

    let json_string = serde_json::to_string_pretty(&Value::Object(json)).unwrap();

    let out_path = std::path::Path::new(&out).join([file_name.as_str(), ".json"].concat());

    std::fs::write(out_path, json_string).unwrap();
}
