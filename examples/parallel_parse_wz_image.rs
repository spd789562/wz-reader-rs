use std::thread;
use wz_reader::{WzNode, WzNodeArc};


fn main() {
    let node: WzNodeArc = WzNode::from_wz_file(r"D:\MapleStory\Data\UI\UI_000.wz", None, None, None).unwrap().into();


    let mut node_write = node.write().unwrap();
    
    node_write.parse(&node).unwrap();

    let handles = node_write.children.values().map(|node| {
        let node = node.clone();
        thread::spawn(move || {
            node.write().unwrap().parse(&node).unwrap();
        })
    }).collect::<Vec<_>>();

    for t in handles {
        t.join().unwrap();
    }
}