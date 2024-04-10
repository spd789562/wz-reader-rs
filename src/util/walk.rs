
use crate::{WzNodeArc, WzObjectType};

pub fn walk_node(node: &WzNodeArc, force_parse: bool, f: &dyn Fn(&WzNodeArc)) {
    if force_parse {
        node.write().unwrap().parse(&node).unwrap();
    }

    f(&node);

    for child in node.read().unwrap().children.values() {
        walk_node(child, force_parse, f);
    }

    let is_wz_image = matches!(node.read().unwrap().object_type, WzObjectType::Image(_));

    if force_parse && is_wz_image {
        if let Ok(mut node) = node.write() {
            node.unparse().unwrap();
        }
    }
}