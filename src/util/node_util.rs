use crate::{node::Error, WzNode, WzNodeArc, WzNodeCast};
use std::sync::Arc;

/// Just wrap around of `node.write().unwrap().parse(&node)`
pub fn parse_node(node: &WzNodeArc) -> Result<(), Error> {
    node.parse(node)
}

/// Resolve a `_inlink` path, a `_inlink` path always start from a `WzImage`.
pub fn resolve_inlink(path: &str, node: &WzNodeArc) -> Option<WzNodeArc> {
    node.get_parent_wz_image()?.at_path(path)
}

/// Resolve a `_outlink` path, a `_outlink` path always start from Wz's data root(a.k.a `Base.wz`).
pub fn resolve_outlink(path: &str, node: &WzNodeArc, force_parse: bool) -> Option<WzNodeArc> {
    let node = node.get_base_wz_file()?;

    if force_parse {
        node.at_path_parsed(path).ok()
    } else {
        node.at_path(path)
    }
}

/// Make sure WzNode tree's all node has correct parent.
///
/// Safety: If any child is trying to get parent during this process, it might be a undefined behavior.
pub unsafe fn resolve_childs_parent(node: &WzNodeArc) {
    for child in node.children.write().unwrap().values_mut() {
        // Safety:
        unsafe {
            let child_inner = { &mut *(Arc::as_ptr(&child) as *mut WzNode) };
            child_inner.parent = Arc::downgrade(node);
        }
        resolve_childs_parent(child);
    }
}

/// Get resolved uol path, it will resolve `..` and `.` in path.
pub fn get_resolved_uol_path(path: &str, uol_path: &str) -> String {
    let mut pathes: Vec<&str> = path.split('/').collect();
    /* uol path always start at parent */
    pathes.pop();
    for p in uol_path.split('/') {
        if p == ".." && !pathes.is_empty() {
            pathes.pop();
        } else {
            pathes.push(p);
        }
    }
    pathes.join("/")
}

/// Make a uol node become valid node, second argument is optional,
/// it prevent the parent is the WzImage while it currently parsing causing the deadlock.
pub fn resolve_uol(node: &WzNodeArc, wz_image: Option<&WzNode>) {
    let node_parent = node.parent.upgrade().unwrap();

    if let Some(ref mut uol_target_path) = node.try_as_uol().and_then(|s| s.get_string().ok()) {
        let mut pathes = uol_target_path.split('/');

        let first = node.at_relative("..");

        let uol_target = if let Some(first) = first {
            pathes.try_fold(first, |node, name| node.at_relative(name))
        } else {
            None
        };

        if let Some(target_node) = uol_target {
            let node_name = node.name.clone();
            if let Some(origin) = node_parent.children.write().unwrap().get_mut(&node_name) {
                let _ = std::mem::replace(origin, target_node);
            }
        }
    }
}

/// Get image node in the way, and return the rest of path.
pub fn get_image_node_from_path<'a>(
    node: &'_ WzNodeArc,
    path: &'a str,
) -> Option<(WzNodeArc, &'a str)> {
    if path.is_empty() {
        return None;
    }

    if path.contains(".img") {
        let mut pathes = path.split_inclusive(".img");
        let img_path = pathes.next()?;
        let rest_path = pathes.next()?.strip_prefix('/')?;

        let image_node = node.at_path(img_path)?;

        return Some((image_node, rest_path));
    }

    let mut node = node.clone();
    let mut slash_index = 0;
    for split_path in path.split('/') {
        let target = node.at(split_path);
        if let Some(target) = target {
            node = target;
            slash_index += split_path.len() + 1;
            if node.try_as_image().is_some() {
                let rest = path.split_at(slash_index).1;
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
    let (image_node, rest_path) = get_image_node_from_path(root, path)?;
    let image = image_node.try_as_image()?;

    if *image.is_parsed.lock().unwrap() {
        image_node.at_path(rest_path)
    } else {
        image.at_path(rest_path).ok()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        property::{resolve_string_from_node, WzString, WzValue},
        WzDirectory, WzFile, WzImage, WzNode, WzObjectType,
    };

    /// construct base node tree
    ///
    /// Base
    /// |- dir
    ///    |- test1.img
    ///       |- 1-dep1
    ///          |- 1-dep2
    ///       |- 2-dep1
    ///          |- 2-dep2
    ///             |- _inlink
    ///             |- _outlink
    ///             |- uol
    ///    |- test2.img
    ///       |- child1
    ///       |- child2
    ///    |- test3.img
    ///       |- orphan1
    ///       |- orphan2
    fn setup_node_tree() -> WzNodeArc {
        let root = WzNode::from_str("Base", WzFile::default(), None).into_lock();
        let dir = WzNode::from_str("dir", WzDirectory::default(), Some(&root)).into_lock();

        let img1 = {
            let img = WzImage::default();
            *img.is_parsed.lock().unwrap() = true;
            WzNode::from_str("test1.img", img, Some(&dir))
        }
        .into_lock();

        let img2 = {
            let img = WzImage::default();
            *img.is_parsed.lock().unwrap() = true;
            WzNode::from_str("test2.img", img, Some(&dir))
        }
        .into_lock();

        let img3 = {
            let img = WzImage::default();
            *img.is_parsed.lock().unwrap() = true;
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
        let image1child21uol = WzNode::from_str(
            "uol",
            WzObjectType::Value(WzValue::UOL(WzString::from_str(
                "../../1-dep1/1-dep2",
                [0, 0, 0, 0],
            ))),
            Some(&img1child21),
        )
        .into_lock();

        let img2child1 = WzNode::from_str("child1", 1, Some(&img2)).into_lock();
        let img2child2 = WzNode::from_str("child2", 2, Some(&img2child1)).into_lock();

        // make those orphan but also is test3.img's child
        let img3child1 = WzNode::from_str("orphan1", 1, None).into_lock();
        let img3child2 = WzNode::from_str("orphan2", 1, None).into_lock();

        root.add(&dir);

        dir.add(&img1);
        dir.add(&img2);
        dir.add(&img3);

        img1.add(&img1child1);
        img1child1.add(&img1child11);
        img1.add(&img1child2);
        img1child2.add(&img1child21);
        img1child21.add(&img1child21inlink);
        img1child21.add(&img1child21outlink);
        img1child21.add(&image1child21uol);

        img2.add(&img2child1);
        img2child1.add(&img2child2);

        img3.add(&img3child1);
        img3child1.add(&img3child2);

        root
    }

    #[test]
    fn test_resolve_inlink() {
        let root = setup_node_tree();

        let node = root.at_path("dir/test1.img/2-dep1/2-dep2/_inlink").unwrap();
        let inlink = resolve_string_from_node(&node).unwrap();

        let inlink_target = resolve_inlink(&inlink, &node);

        assert!(inlink_target.is_some());

        let inlink_target = inlink_target.unwrap();

        assert_eq!(inlink_target.name.as_str(), "1-dep2");
    }

    #[test]
    fn test_resolve_outlink() {
        let root = setup_node_tree();

        let node = root
            .at_path("dir/test1.img/2-dep1/2-dep2/_outlink")
            .unwrap();
        let outlink = resolve_string_from_node(&node).unwrap();

        println!("{:?}", outlink);

        let outlink_target = resolve_outlink(&outlink, &node, false);

        assert!(outlink_target.is_some());

        let outlink_target = outlink_target.unwrap();

        assert_eq!(outlink_target.name.as_str(), "child2");
    }

    #[test]
    fn test_resolve_childs_parent() {
        let root = setup_node_tree();

        let node = root.at_path("dir/test3.img").unwrap();

        unsafe {
            resolve_childs_parent(&node);
        }

        let child1 = node.at("orphan1").unwrap();

        let child1_parent = child1.parent.upgrade().unwrap();

        assert_eq!(child1_parent.name.as_str(), "test3.img");

        let child2 = child1.at("orphan2").unwrap();

        let child2_parent = child2.parent.upgrade().unwrap();

        assert_eq!(child2_parent.name.as_str(), "orphan1");
    }

    #[test]
    fn test_get_image_node_from_path() {
        let root = setup_node_tree();

        let find_result = get_image_node_from_path(&root, "dir/test1.img/2-dep1/2-dep2/_outlink");

        assert!(find_result.is_some());

        let (node, rest) = find_result.unwrap();

        assert_eq!(node.name.as_str(), "test1.img");
        assert_eq!(rest, "2-dep1/2-dep2/_outlink");
    }

    #[test]
    fn test_get_node_without_parse() {
        let root = setup_node_tree();

        let target_node = get_node_without_parse(&root, "dir/test1.img/2-dep1/2-dep2/_outlink");

        assert!(target_node.is_some());

        let target_node = target_node.unwrap();

        assert_eq!(target_node.name.as_str(), "_outlink");
    }

    #[test]
    fn test_get_resolved_uol_path() {
        let path = "dir/test1.img/2-dep1/2-dep2";
        let uol_path = "../1-dep1/1-dep2";

        let resolved = get_resolved_uol_path(path, uol_path);

        assert_eq!(&resolved, "dir/test1.img/1-dep1/1-dep2");
    }

    #[test]
    fn test_resolve_uol() {
        let root = setup_node_tree();

        let uol_node = root.at_path("dir/test1.img/2-dep1/2-dep2/uol").unwrap();

        resolve_uol(&uol_node, None);

        let new_uol_node = root.at_path("dir/test1.img/2-dep1/2-dep2/uol").unwrap();

        assert_eq!(new_uol_node.name.as_str(), "1-dep2");
        assert_eq!(
            new_uol_node.get_full_path(),
            "Base/dir/test1.img/1-dep1/1-dep2"
        );
    }
}
