
use wz_reader::{WzNode};
use wz_reader::property::{get_image};
use wz_reader::util::{resolve_base, walk_node};

fn main() {
    let node = resolve_base(r"D:\MapleStoryV257\MapleStoryV257\Data\Base\Base.wz").unwrap();

    let node = node.read().unwrap().at("Etc").unwrap();

    let other_node = node.read().unwrap().at("BossDunkel.img").unwrap();

    {
        let before = std::time::Instant::now();

        other_node.write().unwrap().parse(&other_node).unwrap();

        println!("parse time: {:?}", before.elapsed());
    }


    {
        let before = std::time::Instant::now();

        let child = other_node.read().unwrap().at_path("AreaWarning/7/areaWarning/2");

        if let Some(child) = child {
            println!("node current path: {}", child.read().unwrap().get_full_path());
            let image = get_image(&child).unwrap();
            image.save("test.png").unwrap();
        }
        
        println!("walk time: {:?}", before.elapsed());
    }

    // let node = resolve_base::<WzNodeArc>(r"D:\MapleStoryV257\MapleStoryV257\Data\Base\Base.wz").unwrap();

    // let sound_node = node.at_path("Character/Accessory").unwrap();

    // walk_node_arc(sound_node, true, &|node| {
    //     if matches!(node.read().unwrap().property_type, WzPropertyType::Convex) {
    //         println!("node current path: {}", node.get_full_path());
    //     }
    // });
}
