use wz_reader::NodeMethods;
use wz_reader::arc::WzNodeArc;
use wz_reader::util::{resolve_base, resolve_root_wz_file_dir, walk_node_arc};

fn main() {
    let save_sound_fn = |node: WzNodeArc| {
        if node.is_png() {
            node.save_sound("./sounds", None).unwrap();
        }
    };

    /* resolve single wz file */
    let node = WzNodeArc::new_wz_file(r"D:\MapleStory\Data\Sound\Sound_000.wz", None);

    walk_node_arc(node, true, &save_sound_fn);

    /* resolve from base.wz */
    let base_node = resolve_base::<WzNodeArc>(r"D:\MapleStory\Data\Base.wz").unwrap();

    /* it't same as below method */
    let sound_node = base_node.at("Sound").unwrap();
    walk_node_arc(sound_node, true, &save_sound_fn);

    /* resolve whole wz folder */
    let root_node = resolve_root_wz_file_dir::<WzNodeArc>(r"D:\MapleStory\Data\Sound\Sound.wz", None).unwrap();

    walk_node_arc(root_node, true, &save_sound_fn);
}