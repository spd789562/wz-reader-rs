
use wz_reader::NodeMethods;
use wz_reader::arc::WzNodeArc;
use wz_reader::property::WzPropertyType;
use wz_reader::util::{resolve_base, walk_node_arc};

fn main() {
    let node = resolve_base::<WzNodeArc>(r"D:\MapleStoryV257\MapleStoryV257\Data\Base\Base.wz").unwrap();

    let sound_node = node.at_path("Character/Accessory").unwrap();

    walk_node_arc(sound_node, true, &|node| {
        if matches!(node.read().unwrap().property_type, WzPropertyType::Convex) {
            println!("node current path: {}", node.get_full_path());
        }
    });
}
