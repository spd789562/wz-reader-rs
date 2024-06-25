use std::path::PathBuf;

use wz_reader::property::get_image;
use wz_reader::util::walk_node;
use wz_reader::{WzNode, WzNodeArc, WzNodeCast};

// usage:
//   cargo run --example parse_single_img_file -- "path/to/some.img"
//   cargo run --example parse_single_img_file -- "D:\Path\To\some.img"
fn main() {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().expect("Need .wz file as 1st argument");
    let out_dir: PathBuf = args.next().expect("Need out dir as 2nd argument").into();
    /* resolve single img file */
    let node: WzNodeArc = WzNode::from_img_file(path, None, None).unwrap().into();

    walk_node(&node, true, &|node: &WzNodeArc| {
        let node_read = node.read().unwrap();
        if node_read.try_as_png().is_some() {
            let image = get_image(&node).unwrap().into_rgba8();
            let save_name = [&node_read.get_full_path().replace("/", "-"), ".png"].concat();
            let out_path = out_dir.join(save_name);
            image.save(out_path).unwrap();
        }
    });
}
