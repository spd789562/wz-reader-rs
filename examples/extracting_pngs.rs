use wz_reader::property::get_image;
use wz_reader::util::{resolve_base, resolve_root_wz_file_dir, walk_node};
use wz_reader::{WzNode, WzNodeArc, WzNodeCast};

// usage:
//   cargo run --example extracting_pngs --features "image/png" -- "path/to/Base.wz" "output/path"
//   cargo run --example extracting_pngs --features "image/png" -- single "D:\Path\To\Base.wz" ".\output"
//   cargo run --example extracting_pngs --features "image/png" -- base "D:\Path\To\Base.wz" ".\output"
//   cargo run --example extracting_pngs --features "image/png" -- folder "D:\Path\To\Base.wz" ".\output"
fn main() {
    let mut args = std::env::args_os().skip(1);
    let method = args
        .next()
        .expect("Need method (single/base/folder) as 1st arg");
    let path = args.next().expect("Need path to wz file as 2nd arg");
    let out = args.next().expect("Need out dir as 3rd arg");
    let out_path_string = out
        .into_string()
        .expect("unable to convert the out path to string");

    let save_image_fn = |node: &WzNodeArc| {
        let node_read = node.read().unwrap();
        if node_read.try_as_png().is_some() {
            let image = get_image(&node).unwrap();
            /* the name of image is easily got conflect */
            let save_name = node_read.get_full_path().replace("/", "-");
            /* resolving image will auto resolve image from _inlink and _outlink */
            image
                .save(format!("{out_path_string}/{save_name}.png"))
                .unwrap();
        }
    };

    match method.as_encoded_bytes() {
        b"single" => {
            /* resolve single wz file */
            let node: WzNodeArc = WzNode::from_wz_file(path, None).unwrap().into();

            walk_node(&node, true, &save_image_fn);
        }
        b"base" => {
            /* resolve from base.wz */
            let base_node = resolve_base(&path, None).unwrap();

            /* this will take millions years */
            walk_node(&base_node, true, &save_image_fn);
        }
        b"folder" => {
            /* resolve whole wz folder */
            let root_node = resolve_root_wz_file_dir(&path, None).unwrap();

            walk_node(&root_node, true, &save_image_fn);
        }
        _ => eprintln!("Invalid method"),
    }
}
