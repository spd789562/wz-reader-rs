use wz_reader::util::walk_node;
use wz_reader::{WzNode, WzNodeArc};

fn main() {
    /* resolve single wz file */
    let node: WzNodeArc = WzNode::from_wz_file(r"D:\MapleStory\Data\UI\UI_000.wz", None)
        .unwrap()
        .into();

    /*
        when you know exactly know what version is, consider using WzNode::from_wz_file_full
    */

    walk_node(&node, true, &|node: &WzNodeArc| {
        println!("{}", node.read().unwrap().get_full_path());
    });
}
