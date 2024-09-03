use std::fs::File;
use std::io::Write;
use wz_reader::util::{resolve_base, walk_node};
use wz_reader::version::WzMapleVersion;
use wz_reader::WzNodeCast;

// usage:
//   cargo run --example extracting_lua -- "path/to/Base.wz" "output"
//   cargo run --example extracting_lua -- "D:\Path\To\Base.wz" "./output"
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let base_path = args.get(1).expect("missing base path");
    let out_path = args.get(2).expect("missing target name");
    let base_node = resolve_base(&base_path, Some(WzMapleVersion::BMS)).unwrap();

    let start = std::time::Instant::now();

    let script_node = base_node
        .at_path("Etc/Script")
        .expect("script node not found");

    walk_node(&script_node, true, &|node| {
        if let Some(lua_node) = node.try_as_lua() {
            let result = lua_node.extract_lua();
            if let Ok(lua_text) = result {
                let lua_file_name = node.parent.upgrade().unwrap().name.clone();
                let lua_save_path = format!("{}/{}", out_path, lua_file_name);
                let mut file = File::create(lua_save_path).unwrap();
                file.write_all(lua_text.as_bytes()).unwrap();
            } else {
                println!("failed to extract lua from {:?}", node.get_full_path());
            }
        }
    });

    println!("total time: {:?}", start.elapsed());
}
