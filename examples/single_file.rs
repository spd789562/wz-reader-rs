use wz_reader::util::walk_node;
use wz_reader::{WzNode, WzNodeArc};

// usage:
//   cargo run --example single_file -- "path/to/some.wz"
//   cargo run --example single_file -- "D:\Path\To\some.wz"
fn main() {
    let path = std::env::args_os()
        .nth(1)
        .expect("Need .wz file as argument");
    /* resolve single wz file */
    /*
        when you know exactly know what version is, consider using WzNode::from_wz_file_full
    */
    let node: WzNodeArc = WzNode::from_wz_file(path, None).unwrap().into();

    walk_node(&node, true, &|node: &WzNodeArc| {
        println!("{}", node.read().unwrap().get_full_path());
    });
}
