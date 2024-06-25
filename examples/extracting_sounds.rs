use wz_reader::util::{resolve_base, resolve_root_wz_file_dir, walk_node};
use wz_reader::{WzNode, WzNodeArc, WzNodeCast};

// usage:
//   cargo run --example extracting_sounds -- (single|base|folder) "path/to/Base.wz" "output/path"
//   cargo run --example extracting_sounds -- single "D:\Path\To\Base.wz" "./output"
//   cargo run --example extracting_sounds -- base "D:\Path\To\Base.wz" "./output"
//   cargo run --example extracting_sounds -- folder "D:\Path\To\Base.wz" "./output"
fn main() {
    let mut args = std::env::args_os().skip(1);
    let method = args
        .next()
        .expect("Need method (single/base/folder) as 1st arg");
    let path = args.next().expect("Need path to wz file as 2nd arg");
    let out = args.next().expect("Need out dir as 3rd arg");
    let save_sound_fn = |node: &WzNodeArc| {
        let node_read = node.read().unwrap();
        if let Some(sound) = node_read.try_as_sound() {
            let path = std::path::Path::new(&out).join(node_read.name.as_str());
            if sound.save(path).is_err() {
                println!("failed to extract sound: {}", node_read.get_full_path());
            }
        }
    };

    match method.as_encoded_bytes() {
        b"single" => {
            /* resolve single wz file */
            let node: WzNodeArc = WzNode::from_wz_file(path, None).unwrap().into();

            walk_node(&node, true, &save_sound_fn);
        }
        b"base" => {
            /* resolve from base.wz */
            let base_node = resolve_base(&path, None).unwrap();

            /* it's same as below method */
            let sound_node = base_node.read().unwrap().at("Sound").unwrap();
            walk_node(&sound_node, true, &save_sound_fn);
        }
        b"folder" => {
            /* resolve whole wz folder */
            let root_node = resolve_root_wz_file_dir(&path, None).unwrap();

            walk_node(&root_node, true, &save_sound_fn);
        }
        _ => eprintln!("Invalid method"),
    }
}
