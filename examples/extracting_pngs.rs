use wz_reader::NodeMethods;
use wz_reader::arc::WzNodeArc;
use wz_reader::util::{resolve_base, resolve_root_wz_file_dir, walk_node_arc};

fn main() {
    let save_image_fn = |node: WzNodeArc| {
        if node.is_png() {
            /* the name of image is easily got conflect */
            let save_name = node.get_full_path().replace("/", "-");
            /* resolving image will auto resolve image from _inlink and _outlink */
            node.save_image("./images", Some(&save_name)).unwrap();
        }
    };
    
    /* resolve single wz file */
    let node = WzNodeArc::new_wz_file(r"D:\MapleStory\Data\Npc\_Canvas\_Canvas000.wz", None);

    walk_node_arc(node, true, &save_image_fn);

    /* resolve from base.wz */
    let base_node = resolve_base::<WzNodeArc>(r"D:\MapleStory\Data\Base.wz").unwrap();

    /* this will take millions years */
    walk_node_arc(base_node, true, &save_image_fn);

    /* resolve whole wz folder */
    let root_node = resolve_root_wz_file_dir::<WzNodeArc>(r"D:\MapleStory\Data\Npc\_Canvas\_Canvas.wz", None).unwrap();

    walk_node_arc(root_node, true, &save_image_fn);
}