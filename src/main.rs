
use wz_reader::{WzImage, WzNode, WzNodeArc, WzObjectType};
use wz_reader::version::WzMapleVersion;
use wz_reader::property::{get_image, WzValue};
use wz_reader::util::{resolve_base, walk_node};

fn main() {
    let start = std::time::Instant::now();
    // let node: WzNodeArc = WzNode::from_wz_file(r"C:\Users\07665.leo.lin\Downloads\test113.wz", Some(WzMapleVersion::BMS), None, None).unwrap().into();
    let node = resolve_base(r"D:\MapleStoryV257\MapleStoryV257\Data\Base\Base.wz", None).unwrap();

    println!("resolve time: {:?}", start.elapsed());

    // let node = node.read().unwrap().at_path("Etc/Android/0027.img").unwrap();

    // {
    //     let before = std::time::Instant::now();

    //     if let WzObjectType::Image(wz_image) = &node.read().unwrap().object_type {
    //         wz_image.at_path("action/alert/0/action", &node).unwrap();
    //     }

    //     println!("get time: {:?}", before.elapsed());
    // }

    // {
    //     let before = std::time::Instant::now();

    //     node.write().unwrap().parse(&node).unwrap();

    //     println!("parse time: {:?}", before.elapsed());
    // }

    // {
    //     let before = std::time::Instant::now();

    //     node.read().unwrap().at_path("action/alert/0/action").unwrap();

    //     println!("get time: {:?}", before.elapsed());
    // }


    // {
    //     let before = std::time::Instant::now();
    //     walk_node(&node, true, &|node| {
    //         let node_read = node.read().unwrap();
    //     });
    //     println!("walk time: {:?}", before.elapsed());
    // }

    // let node = resolve_base::<WzNodeArc>(r"D:\MapleStoryV257\MapleStoryV257\Data\Base\Base.wz").unwrap();

    // let sound_node = node.at_path("Character/Accessory").unwrap();

    // walk_node_arc(sound_node, true, &|node| w{
    //     if matches!(node.read().unwrap().property_type, WzPropertyType::Convex) {
    //         println!("node current path: {}", node.get_full_path());
    //     }
    // });
}
