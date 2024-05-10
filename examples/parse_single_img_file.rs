use wz_reader::property::get_image;
use wz_reader::util::walk_node;
use wz_reader::version::WzMapleVersion;
use wz_reader::{WzNode, WzNodeArc, WzNodeCast};

fn main() {
    /* resolve single img file */
    let node: WzNodeArc = WzNode::from_img_file(
        r"D:\MapleStory\Data\Item\_Canvas\_Canvas\5000024.img",
        Some(WzMapleVersion::BMS),
        None,
    )
    .unwrap()
    .into();

    walk_node(&node, true, &|node: &WzNodeArc| {
        let node_read = node.read().unwrap();
        if node_read.try_as_png().is_some() {
            let image = get_image(&node).unwrap();
            let save_name = node_read.get_full_path().replace("/", "-");
            image.save(format!("./images/{save_name}.png")).unwrap();
        }
    });
}
