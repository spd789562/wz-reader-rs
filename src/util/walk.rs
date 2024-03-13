use crate::arc::WzNodeArc;
use crate::rc::WzNodeRc;
use crate::{NodeMethods, WzObjectType};

pub fn walk_node_rc(node: WzNodeRc, force_parse: bool, f: &dyn Fn(WzNodeRc)) {
    if force_parse {
        node.parse().unwrap();
    }

    f(node.clone());

    for child in node.borrow().children.values() {
        walk_node_rc(child.clone(), force_parse, f);
    }

    if force_parse && node.borrow().object_type == WzObjectType::Image {
        node.unparse_image().unwrap();
    }
}

pub fn walk_node_arc(node: WzNodeArc, force_parse: bool, f: &dyn Fn(WzNodeArc)) {
    if force_parse {
        node.parse().unwrap();
    }

    f(node.clone());

    for child in node.read().unwrap().children.values() {
        walk_node_arc(child.clone(), force_parse, f);
    }

    if force_parse && node.read().unwrap().object_type == WzObjectType::Image {
        node.unparse_image().unwrap();
    }
}