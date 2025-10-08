use wz_reader::util::walk_node;
use wz_reader::{WzNode, WzNodeArc};

type Error = Box<dyn std::error::Error>;
type Result<T> = std::result::Result<T, Error>;

// usage:
//   cargo run --example parse_ms_file -- "path/to/file.ms"
//   cargo run --example parse_ms_file -- "D:\Path\To\file.ms"
fn main() -> Result<()> {
    let args = std::env::args_os().collect::<Vec<_>>();
    let base_path = args.get(1).expect("missing ms file path");

    let ms_file_node = WzNode::from_ms_file(base_path, None)?.into_lock();

    ms_file_node.write().unwrap().parse(&ms_file_node)?;

    walk_node(&ms_file_node, true, &|node: &WzNodeArc| {
        if node.read().unwrap().name.contains("text.txt") {
            println!("{}", node.read().unwrap().get_full_path());
        }
    });

    Ok(())
}
