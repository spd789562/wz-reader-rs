use wz_reader::{WzNode, WzNodeArc, WzNodeCast};
use wz_reader::util::{resolve_base, resolve_root_wz_file_dir, walk_node};

fn main() {
    let save_sound_fn = |node: &WzNodeArc| {
        let node_read = node.read().unwrap();
        if let Some(sound) = node_read.try_as_sound() {
            let path = std::path::Path::new("./sounds").join(node_read.name.as_str());
            if sound.save(path).is_err() {
                println!("failed to extract sound: {}", node_read.get_full_path());
            }
        }
    };

    /* resolve single wz file */
    let node: WzNodeArc = WzNode::from_wz_file(r"D:\MapleStory\Data\Sound\Sound_000.wz", None, None, None).unwrap().into();

    walk_node(&node, true, &save_sound_fn);

    /* resolve from base.wz */
    let base_node = resolve_base(r"D:\MapleStory\Data\Base.wz", None).unwrap();

    /* it't same as below method */
    let sound_node = base_node.read().unwrap().at("Sound").unwrap();
    walk_node(&sound_node, true, &save_sound_fn);

    /* resolve whole wz folder */
    let root_node = resolve_root_wz_file_dir(r"D:\MapleStory\Data\Sound\Sound.wz", None, None, None).unwrap();

    walk_node(&root_node, true, &save_sound_fn);
}