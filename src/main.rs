
use wz_reader::NodeMethods;
use wz_reader::arc::WzNodeArc;
use wz_reader::util::{resolve_base, walk_node_arc};
fn main() {
    let node = resolve_base::<WzNodeArc>(r"D:\MapleStory\Data\Base\Base.wz").unwrap();

    let sound_node = node.at_path("Sound/Bgm08.img").unwrap();

    walk_node_arc(sound_node, true, &|node| {
        println!("node current path: {}, type: {:?}", node.get_full_path(), node.read().unwrap().object_type);
    });
}
