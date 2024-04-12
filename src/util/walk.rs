
use crate::{WzNodeArc, WzObjectType};

pub fn walk_node(node: &WzNodeArc, force_parse: bool, f: &dyn Fn(&WzNodeArc)) {
    if force_parse {
        node.write().unwrap().parse(node).unwrap();
    }

    f(node);

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

#[cfg(test)]
mod test {
    use super::*;
    use crate::{WzObjectType, WzNode, property::{WzValue, WzSubProperty}};

    fn generate_mock_node() -> WzNodeArc {
        let root = WzNode::new("root".to_string(), WzObjectType::Property(WzSubProperty::Property), None).into_lock();

        let child1 = WzNode::new("child1".to_string(), WzObjectType::Value(WzValue::Int(1)), Some(&root)).into_lock();
        let child2 = WzNode::new("child2".to_string(), WzObjectType::Value(WzValue::Int(2)), Some(&root)).into_lock();

        WzNode::new("child11".to_string(), WzObjectType::Value(WzValue::Int(11)), Some(&child1)).into_lock();
        WzNode::new("child12".to_string(), WzObjectType::Value(WzValue::Int(12)), Some(&child1)).into_lock();

        WzNode::new("child21".to_string(), WzObjectType::Value(WzValue::Int(21)), Some(&child2)).into_lock();
        WzNode::new("child22".to_string(), WzObjectType::Value(WzValue::Int(22)), Some(&child2)).into_lock();

        root
    }

    #[test]
    fn test_walk_node() {
        let root = generate_mock_node();

        let pathes = std::collections::HashSet::from([
            "root",
            "root/child1",
            "root/child1/child11",
            "root/child1/child12",
            "root/child2",
            "root/child2/child21",
            "root/child2/child22",
        ]);
        
        walk_node(&root, false, &|node| {
            let node_read = node.read().unwrap();
            assert!(pathes.contains(node_read.get_full_path().as_str()));
        });
    }
}