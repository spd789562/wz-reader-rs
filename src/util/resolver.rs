use std::io;
use std::fs::DirEntry;
use std::path::Path;
use std::sync::Arc;
use crate::{WzNode, WzNodeArc};

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

pub fn resolve_root_wz_file_dir(dir: &str, parent: Option<&WzNodeArc>) -> Result<WzNodeArc, io::Error> {
    let root_node: WzNodeArc = WzNode::from_wz_file(dir, parent).unwrap().into();
    let wz_dir = Path::new(dir).parent().unwrap();

    {
        let mut root_node_write = root_node.write().unwrap();
    
        root_node_write.parse(&root_node).unwrap();
    
        for entry in wz_dir.read_dir()? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let name = entry.file_name();
    
            if file_type.is_dir() && root_node_write.at(name.to_str().unwrap()).is_some() {
                if let Some(file_path) = get_root_wz_file_path(&entry) {
                    let dir_node = resolve_root_wz_file_dir(&file_path, Some(&root_node))?;
                    
                    /* replace the original one */
                    root_node_write.children.insert(name.to_str().unwrap().to_string(), dir_node);
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
    
                let node = WzNode::from_wz_file(file_path.to_str().unwrap(), None).unwrap().into_lock();
    
                let mut node_write = node.write().unwrap();
    
                if node_write.parse(&node).is_ok() {
                    for (name, child) in node_write.children.drain() {
                        {
                            child.write().unwrap().parent = Arc::downgrade(&root_node);
                        }
                        root_node_write.children.insert(name, child);
                    }
                }
            }
        }
    }

    Ok(root_node)
}

pub fn resolve_base(path: &str) -> Result<WzNodeArc, io::Error> {
    if !path.ends_with("Base.wz") {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a Base.wz"));
    }

    let base_node = resolve_root_wz_file_dir(path, None)?;

    {
        let mut base_write = base_node.write().unwrap();
    
        let wz_root_path = Path::new(path).parent().unwrap().parent().unwrap();
    
        for item in wz_root_path.read_dir()? {
            let dir = item?;
            let file_name = dir.file_name();
    
            /* we need aquire wirte lock to "add_node_child" so need release read lock here */
            let has_dir = base_write.at(file_name.to_str().unwrap()).is_some();
    
            if has_dir {
                let wz_path = get_root_wz_file_path(&dir);
    
                if let Some(file_path) = wz_path {
                    let dir_node = resolve_root_wz_file_dir(&file_path, Some(&base_node))?;

                    /* replace the original one */
                    base_write.children.insert(file_name.to_str().unwrap().to_string(), dir_node);
                }
            }
        }
    }

    Ok(base_node)
}
