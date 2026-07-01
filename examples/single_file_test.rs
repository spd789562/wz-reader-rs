use wz_reader::util::{node_util, walk_node};
use wz_reader::{WzNode, WzNodeArc, WzNodeCast};

// usage:
//   cargo run --example single_file -- "path/to/some.wz"
//   cargo run --example single_file -- "D:\Path\To\some.wz"
fn main() {
    let path = std::env::args_os()
        .nth(1)
        .expect("Need .wz file as argument");
    /* resolve single wz file */
    let test_path_1 = String::from("/Users/leanlin/Downloads/String_000(1).wz");
    let test_path_2 = String::from("/Users/leanlin/Downloads/Item_000.wz");
    /*
        when you know exactly know what version is, consider using WzNode::from_wz_file_full
    */
    let node: WzNodeArc = WzNode::from_wz_file(test_path_1, None).unwrap().into();
    let node2: WzNodeArc = WzNode::from_wz_file(test_path_2, None).unwrap().into();

    let start = std::time::Instant::now();
    let _ = node_util::parse_node(&node);
    println!("parse node 1 time: {:?}", start.elapsed());
    let _ = node_util::parse_node(&node2);
    println!("parse node 2 time: {:?}", start.elapsed());

    // walk_node(&node, true, &|node: &WzNodeArc| {
    //     if let Some(string) = node.read().unwrap().try_as_string() {
    //         // println!(
    //         //     "{}, {}",
    //         //     node.read().unwrap().get_full_path(),
    //         //     string.get_string().unwrap()
    //         // );
    //     }
    // });
}
