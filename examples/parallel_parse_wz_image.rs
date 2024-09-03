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

    node.parse(&node).unwrap();

    let handles = node
        .children
        .read()
        .unwrap()
        .values()
        .map(|node| {
            let node = node.clone();
            thread::spawn(move || {
                node.parse(&node).unwrap();
            })
        })
        .collect::<Vec<_>>();

    for t in handles {
        t.join().unwrap();
    }
}
