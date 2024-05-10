use wz_reader::property::get_image;
use wz_reader::util::{resolve_base, resolve_root_wz_file_dir, walk_node};
use wz_reader::{WzNode, WzNodeArc, WzNodeCast};

fn main() {
    let save_image_fn = |node: &WzNodeArc| {
        let node_read = node.read().unwrap();
        if node_read.try_as_png().is_some() {
            let image = get_image(&node).unwrap();
            /* the name of image is easily got conflect */
            let save_name = node_read.get_full_path().replace("/", "-");
            /* resolving image will auto resolve image from _inlink and _outlink */
            image.save(format!("./images/{save_name}.png")).unwrap();
        }
    };

    /* resolve single wz file */
    let node: WzNodeArc = WzNode::from_wz_file(
        r"D:\MapleStory\Data\Npc\_Canvas\_Canvas000.wz",
        None,
        None,
        None,
    )
    .unwrap()
    .into();

    walk_node(&node, true, &save_image_fn);

    /* resolve from base.wz */
    let base_node = resolve_base(r"D:\MapleStory\Data\Base.wz", None).unwrap();

    /* this will take millions years */
    walk_node(&base_node, true, &save_image_fn);

    /* resolve whole wz folder */
    let root_node = resolve_root_wz_file_dir(
        r"D:\MapleStory\Data\Npc\_Canvas\_Canvas.wz",
        None,
        None,
        None,
    )
    .unwrap();

    walk_node(&root_node, true, &save_image_fn);
}
