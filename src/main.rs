
use wz_reader::{WzObjectType, NodeMethods};
use wz_reader::arc::WzNodeArc;
use std::io;
use std::{fs::DirEntry, path::Path};

fn walk_node(node: WzNodeArc) {
    // println!("node current path: {}, type: {:?}", node.get_full_path(), node.read().unwrap().object_type);

    if let Err(e) = node.parse() {
        println!("parse error: {}", e.to_string());
    }

    for (_, child) in node.read().unwrap().children.iter() {
        if child.read().unwrap().object_type == WzObjectType::Property {
            continue;
        }
        walk_node(child.clone());

        let is_img =  { child.read().unwrap().object_type == WzObjectType::Image };

        /* clear ImageNode after testing parse */
        if is_img {
            {
                let mut child = child.write().unwrap();
                child.children.clear();
            }
            child.update_parse_status(false);
        }
    }
}


fn get_root_wz_file_path(dir: &DirEntry) -> Option<String> {
    let dir_name = dir.file_name();
    let mut inner_wz_name = dir_name.to_str().unwrap().to_string();
    inner_wz_name.push_str(".wz");
    let inner_wz_path = dir.path().join(inner_wz_name);

    if inner_wz_path.try_exists().is_ok() {
        return Some(inner_wz_path.to_str().unwrap().to_string());
    }

    None
}

fn resolve_root_wz_file_dir(dir: String, parent: Option<&WzNodeArc>) -> Result<WzNodeArc, String> {
    let root_node = WzNodeArc::new_wz_file(&dir, parent);
    let wz_dir = Path::new(&dir).parent().unwrap();

    root_node.parse().unwrap();

    for entry in wz_dir.read_dir().unwrap() {
        let entry = entry.unwrap();
        let file_type = entry.file_type().unwrap();
        let name = entry.file_name();

        if file_type.is_dir() && root_node.at(name.to_str().unwrap()).is_some() {
            let file_path = get_root_wz_file_path(&entry);
            if let Some(file_path) = file_path {
                let dir_node = resolve_root_wz_file_dir(file_path, Some(&root_node)).unwrap();
                
                /* replace the original one */
                root_node.add_node_child(dir_node);
            }
        } else if file_type.is_file() {
            //  check is XXX_nnn.wz
            let file_path = entry.path();
            let file_name = file_path.file_stem().unwrap().to_str().unwrap();

            let splited = file_name.split('_').collect::<Vec<&str>>();

            if splited.len() < 2 {
                continue;
            }

            if splited.last().unwrap().parse::<u16>().is_err() {
                continue;
            }

            let node = WzNodeArc::new_wz_file(file_path.to_str().unwrap(), None);

            if node.parse().is_ok() {
                node.transfer_childs(&root_node);
            }
        }
    }

    Ok(root_node)
}

fn resolve_base(path: &str) -> Result<WzNodeArc, io::Error> {
    if !path.ends_with("Base.wz") {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a Base.wz"));
    }

    let base_node = resolve_root_wz_file_dir(path.to_string(), None).unwrap();

    let wz_root_path = Path::new(path).parent().unwrap().parent().unwrap();

    for item in wz_root_path.read_dir().unwrap() {
        let dir = item.unwrap();
        let file_name = dir.file_name();

        let has_dir = {
            let base_node_read = base_node.read().unwrap();
            base_node_read.children.contains_key(file_name.to_str().unwrap())
        };

        if has_dir {
            let wz_path = get_root_wz_file_path(&dir);

            if let Some(file_path) = wz_path {
                let dir_node = resolve_root_wz_file_dir(file_path, Some(&base_node)).unwrap();
                
                /* replace the original one */
                base_node.add_node_child(dir_node);
            }
        }
    }

    Ok(base_node)
}

fn main() {
    let before_parse_time = std::time::Instant::now();
    let node = resolve_base(r"D:\MapleStory\Data\Base\Base.wz").unwrap();
    let after_parset_time = std::time::Instant::now();
}
