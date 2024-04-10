
use wz_reader::{WzNode, WzObjectType};
use wz_reader::property::{get_image, WzValue};
use wz_reader::util::{resolve_base, walk_node};

fn main() {
    let node = resolve_base(r"D:\MapleStoryV257\MapleStoryV257\Data\Base\Base.wz", None).unwrap();

    let node = node.read().unwrap().at_path("Etc/Script").unwrap();


    {
        let before = std::time::Instant::now();

        node.write().unwrap().parse(&node).unwrap();

        println!("parse time: {:?}", before.elapsed());
    }


    {
        walk_node(&node, true, &|node| {
            let node_read = node.read().unwrap();
            let lua_file_name = node_read.parent.upgrade().unwrap().read().unwrap().name.clone();
            if let WzObjectType::Value(WzValue::Lua(lua)) = &node_read.object_type {
                if let Ok(lua) = lua.extract_lua() {
                    std::fs::write(format!("./{}", lua_file_name), lua).unwrap();
                }
            }
        });
    }

    // let node = resolve_base::<WzNodeArc>(r"D:\MapleStoryV257\MapleStoryV257\Data\Base\Base.wz").unwrap();

    // let sound_node = node.at_path("Character/Accessory").unwrap();

    // walk_node_arc(sound_node, true, &|node| {
    //     if matches!(node.read().unwrap().property_type, WzPropertyType::Convex) {
    //         println!("node current path: {}", node.get_full_path());
    //     }
    // });
}
