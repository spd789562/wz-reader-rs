use wz_reader::util::walk_node;
use wz_reader::{WzNode, WzNodeArc};

fn main() {
    let path = std::env::args_os()
        .nth(1)
        .expect("Need .wz file as argument");
    /* resolve single wz file */
    let node: WzNodeArc = WzNode::from_wz_file(path, None).unwrap().into();

    /*
        when you know exactly know what version is, consider using WzNode::from_wz_file_full
    */

    walk_node(&node, true, &|node: &WzNodeArc| {
        println!("{}", node.read().unwrap().get_full_path());
    });
}
