use std::thread;
use wz_reader::NodeMethods;
use wz_reader::arc::WzNodeArc;

fn main() {
    let node = WzNodeArc::new_wz_file(r"D:\MapleStory\Data\UI\UI_000.wz", None);

    node.parse().unwrap();

    {
        let node = node.read().unwrap();
        let handles = node.children.values().map(|node| {
            let node = node.clone();
            thread::spawn(move || {
                node.parse().unwrap();
            })
        }).collect::<Vec<_>>();

        for t in handles {
            t.join().unwrap();
        }
    }
}