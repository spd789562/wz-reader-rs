use crate::arc::WzNodeArc;
use crate::rc::WzNodeRc;
use crate::NodeMethods;

pub fn walk_node_rc(node: WzNodeRc, force_parse: bool, f: &dyn Fn(WzNodeRc)) {
    if force_parse {
        node.parse().unwrap();
    }

    f(node.clone());

    for child in node.borrow().children.values() {
        walk_node_rc(child.clone(), force_parse, f);
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
}