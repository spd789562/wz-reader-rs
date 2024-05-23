use crate::{node::Error, WzNodeArc, WzNodeCast};
use std::sync::Arc;

/// Just wrap around of `node.write().unwrap().parse(&node)`
pub fn parse_node(node: &WzNodeArc) -> Result<(), Error> {
    node.write().unwrap().parse(node)
}

/// Resolve a `_inlink` path, a `_inlink` path always start from a `WzImage`.
pub fn resolve_inlink(path: &str, node: &WzNodeArc) -> Option<WzNodeArc> {
    let parent_wz_image = node.read().unwrap().get_parent_wz_image()?;
    let parent_wz_image = parent_wz_image.read().unwrap();
    parent_wz_image.at_path(path)
}

/// Resolve a `_outlink` path, a `_outlink` path always start from Wz's data root(a.k.a `Base.wz`).
pub fn resolve_outlink(path: &str, node: &WzNodeArc, force_parse: bool) -> Option<WzNodeArc> {
    let parent_wz_base = node.read().unwrap().get_base_wz_file()?;

    if force_parse {
        parent_wz_base.write().unwrap().at_path_parsed(path).ok()
    } else {
        parent_wz_base.read().unwrap().at_path(path)
    }
}

/// Make sure WzNode tree's all node has correct parent.
pub fn resolve_childs_parent(node: &WzNodeArc) {
    let node_read = node.read().unwrap();
    for child in node_read.children.values() {
        child.write().unwrap().parent = Arc::downgrade(node);
        resolve_childs_parent(child);
    }
}

/// Get image node in the way, and return the rest of path.
pub fn get_image_node_from_path(node: &WzNodeArc, path: &str) -> Option<(WzNodeArc, String)> {
    let mut pathes = path.split('/');
    let mut node = node.clone();
    while let Some(path) = pathes.next() {
        let target = node.read().unwrap().at(path);
        if let Some(target) = target {
            node = target;
            if node.read().unwrap().try_as_image().is_some() {
                let rest = pathes.collect::<Vec<&str>>().join("/");
                return Some((node, rest));
            }
        } else {
            return None;
        }
    }
    None
}

/// get a certain node without parsing all node in the way
pub fn get_node_without_parse(root: &WzNodeArc, path: &str) -> Option<WzNodeArc> {
    let has_img = path.contains(".img");

    if has_img {
        // get the node with name end with `.img`
        // ex: a/b/c.img/d/e -> a/b/c.img
        let mut pathes = path.split_inclusive(".img");
        let img_path = pathes.next()?;
        let rest_path = pathes.next()?.strip_prefix("/")?;

        println!("img_path: {}, rest_path: {}", img_path, rest_path);

        let image_node = root.read().unwrap().at_path(img_path)?;

        let image_read = image_node.read().unwrap();

        // verify the node is a image node
        let image = image_read.try_as_image()?;

        // if it already parsed, use the node method not image method
        if image.is_parsed {
            image_read.at_path(rest_path)
        } else {
            image.at_path(rest_path).ok()
        }
    } else {
        let (image_node, rest_path) = get_image_node_from_path(root, path)?;
        let image_read = image_node.read().unwrap();
        let image = image_read.try_as_image()?;

        if image.is_parsed {
            image_read.at_path(&rest_path)
        } else {
            image.at_path(&rest_path).ok()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        property::{resolve_string_from_node, WzString},
        WzDirectory, WzFile, WzImage, WzNode,
    };

    fn setup_node_tree() -> WzNodeArc {
        let root = WzNode::from_str("Base", WzFile::default(), None).into_lock();
        let dir = WzNode::from_str("dir", WzDirectory::default(), Some(&root)).into_lock();

        let img1 = {
            let mut img = WzImage::default();
            img.is_parsed = true;
            WzNode::from_str("test1.img", img, Some(&dir))
        }
        .into_lock();

        let img2 = {
            let mut img = WzImage::default();
            img.is_parsed = true;
            WzNode::from_str("test2.img", img, Some(&dir))
        }
        .into_lock();

        let img3 = {
            let mut img = WzImage::default();
            img.is_parsed = true;
            WzNode::from_str("test3.img", img, Some(&dir))
        }
        .into_lock();

        let img1child1 = WzNode::from_str("1-dep1", 1, Some(&img1)).into_lock();
        let img1child11 = WzNode::from_str("1-dep2", 2, Some(&img1child1)).into_lock();
        let img1child2 = WzNode::from_str("2-dep1", 1, Some(&img1)).into_lock();
        let img1child21 = WzNode::from_str("2-dep2", 1, Some(&img1child2)).into_lock();
        let img1child21inlink = WzNode::from_str(
            "_inlink",
            WzString::from_str("1-dep1/1-dep2", [0, 0, 0, 0]),
            Some(&img1child21),
        )
        .into_lock();
        let img1child21outlink = WzNode::from_str(
            "_outlink",
            WzString::from_str("dir/test2.img/child1/child2", [0, 0, 0, 0]),
            Some(&img1child21),
        )
        .into_lock();

        let img2child1 = WzNode::from_str("child1", 1, Some(&img2)).into_lock();
        let img2child2 = WzNode::from_str("child2", 2, Some(&img2child1)).into_lock();

        // make those orphan but also is test3.img's child
        let img3child1 = WzNode::from_str("orphan1", 1, None).into_lock();
        let img3child2 = WzNode::from_str("orphan2", 1, None).into_lock();

        root.write().unwrap().add(&dir);

        dir.write().unwrap().add(&img1);
        dir.write().unwrap().add(&img2);
        dir.write().unwrap().add(&img3);

        img1.write().unwrap().add(&img1child1);
        img1child1.write().unwrap().add(&img1child11);
        img1.write().unwrap().add(&img1child2);
        img1child2.write().unwrap().add(&img1child21);
        img1child21.write().unwrap().add(&img1child21inlink);
        img1child21.write().unwrap().add(&img1child21outlink);

        img2.write().unwrap().add(&img2child1);
        img2child1.write().unwrap().add(&img2child2);

        img3.write().unwrap().add(&img3child1);
        img3child1.write().unwrap().add(&img3child2);

        root
    }

    #[test]
    fn test_resolve_inlink() {
        let root = setup_node_tree();

        let node = root
            .read()
            .unwrap()
            .at_path("dir/test1.img/2-dep1/2-dep2/_inlink")
            .unwrap();
        let inlink = resolve_string_from_node(&node).unwrap();

        let inlink_target = resolve_inlink(&inlink, &node);

        assert!(inlink_target.is_some());

        let inlink_target = inlink_target.unwrap();

        assert_eq!(inlink_target.read().unwrap().name.as_str(), "1-dep2");
    }

    #[test]
    fn test_resolve_outlink() {
        let root = setup_node_tree();

        let node = root
            .read()
            .unwrap()
            .at_path("dir/test1.img/2-dep1/2-dep2/_outlink")
            .unwrap();
        let outlink = resolve_string_from_node(&node).unwrap();

        println!("{:?}", outlink);

        let outlink_target = resolve_outlink(&outlink, &node, false);

        assert!(outlink_target.is_some());

        let outlink_target = outlink_target.unwrap();

        assert_eq!(outlink_target.read().unwrap().name.as_str(), "child2");
    }

    #[test]
    fn test_resolve_childs_parent() {
        let root = setup_node_tree();

        let node = root.read().unwrap().at_path("dir/test3.img").unwrap();

        resolve_childs_parent(&node);

        let child1 = node.read().unwrap().at("orphan1").unwrap();

        let child1_parent = child1.read().unwrap().parent.upgrade().unwrap();

        assert_eq!(child1_parent.read().unwrap().name.as_str(), "test3.img");

        let child2 = child1.read().unwrap().at("orphan2").unwrap();

        let child2_parent = child2.read().unwrap().parent.upgrade().unwrap();

        assert_eq!(child2_parent.read().unwrap().name.as_str(), "orphan1");
    }

    #[test]
    fn test_get_image_node_from_path() {
        let root = setup_node_tree();

        let find_result = get_image_node_from_path(&root, "dir/test1.img/2-dep1/2-dep2/_outlink");

        assert!(find_result.is_some());

        let (node, rest) = find_result.unwrap();

        assert_eq!(node.read().unwrap().name.as_str(), "test1.img");
        assert_eq!(rest, "2-dep1/2-dep2/_outlink");
    }

    #[test]
    fn test_get_node_without_parse() {
        let root = setup_node_tree();

        let target_node = get_node_without_parse(&root, "dir/test1.img/2-dep1/2-dep2/_outlink");

        assert!(target_node.is_some());

        let target_node = target_node.unwrap();

        assert_eq!(target_node.read().unwrap().name.as_str(), "_outlink");
    }
}
