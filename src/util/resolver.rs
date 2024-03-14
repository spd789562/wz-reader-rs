use std::io;
use std::fs::DirEntry;
use std::path::Path;
use crate::NodeMethods;

pub fn get_root_wz_file_path(dir: &DirEntry) -> Option<String> {
    let dir_name = dir.file_name();
    let mut inner_wz_name = dir_name.to_str().unwrap().to_string();
    inner_wz_name.push_str(".wz");
    let inner_wz_path = dir.path().join(inner_wz_name);

    if inner_wz_path.try_exists().is_ok() {
        return Some(inner_wz_path.to_str().unwrap().to_string());
    }

    None
}

pub fn resolve_root_wz_file_dir<Node: NodeMethods<Node = Node> + Clone>(dir: &str, parent: Option<&Node>) -> Result<Node, io::Error> {
    let root_node = Node::new_wz_file(&dir, parent);
    let wz_dir = Path::new(dir).parent().unwrap();

    root_node.parse().unwrap();

    for entry in wz_dir.read_dir()? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let name = entry.file_name();

        if file_type.is_dir() && root_node.at(name.to_str().unwrap()).is_ok() {
            if let Some(file_path) = get_root_wz_file_path(&entry) {
                let dir_node = resolve_root_wz_file_dir(&file_path, Some(&root_node))?;
                
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

            let node = Node::new_wz_file(file_path.to_str().unwrap(), None);

            if node.parse().is_ok() {
                node.transfer_childs(&root_node);
            }
        }
    }

    Ok(root_node)
}

pub fn resolve_base<Node: NodeMethods<Node = Node> + Clone>(path: &str) -> Result<Node, io::Error> {
    if !path.ends_with("Base.wz") {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a Base.wz"));
    }

    let base_node = resolve_root_wz_file_dir::<Node>(path, None)?;

    let wz_root_path = Path::new(path).parent().unwrap().parent().unwrap();

    for item in wz_root_path.read_dir()? {
        let dir = item?;
        let file_name = dir.file_name();

        /* we need aquire wirte lock to "add_node_child" so need release read lock here */
        let has_dir = base_node.at(file_name.to_str().unwrap()).is_ok();

        if has_dir {
            let wz_path = get_root_wz_file_path(&dir);

            if let Some(file_path) = wz_path {
                let dir_node = resolve_root_wz_file_dir(&file_path, Some(&base_node))?;
                
                /* replace the original one */
                base_node.add_node_child(dir_node);
            }
        }
    }

    Ok(base_node)
}
