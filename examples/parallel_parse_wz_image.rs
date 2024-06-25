use std::thread;
use wz_reader::{WzNode, WzNodeArc};

// usage:
//   cargo run --example parallel_parse_wz_image -- "path/to/some.wz"
//   cargo run --example parallel_parse_wz_image -- "D:\Path\To\some.wz"
fn main() {
    let path = std::env::args_os()
        .nth(1)
        .expect("Need path to .wz as first argument");
    let node: WzNodeArc = WzNode::from_wz_file(path, None).unwrap().into();

    let mut node_write = node.write().unwrap();

    node_write.parse(&node).unwrap();

    let handles = node_write
        .children
        .values()
        .map(|node| {
            let node = node.clone();
            thread::spawn(move || {
                node.write().unwrap().parse(&node).unwrap();
            })
        })
        .collect::<Vec<_>>();

    for t in handles {
        t.join().unwrap();
    }
}
