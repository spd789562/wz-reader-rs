use wz_reader::NodeMethods;
use wz_reader::arc::WzNodeArc;
use wz_reader::util::walk_node_arc;

fn main() {
    /* resolve single wz file */
    let node = WzNodeArc::new_wz_file(r"D:\MapleStory\Data\UI\UI_000.wz", None);

    walk_node_arc(node, true, &|node: WzNodeArc| {
        println!("{}", node.get_full_path());
    });
}