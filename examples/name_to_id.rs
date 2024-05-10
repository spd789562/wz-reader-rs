use rayon::prelude::*;
use std::sync::Mutex;
use wz_reader::property::string;
use wz_reader::util::{resolve_base, walk_node};
use wz_reader::WzNodeCast;

// usage:
//   cargo run --example id_to_name -- "path/to/Base.wz" "name"
//   cargo run --example id_to_name -- "D:\Path\To\Base.wz" "symbol"
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let base_path = args.get(1).expect("missing base path");
    let item_name = args.get(2).expect("missing target name");
    let base_node = resolve_base(&base_path, None).unwrap();

    let start = std::time::Instant::now();

    let string_nodes = {
        let mut nodes = vec![];
        let base_node = base_node.read().unwrap();

        base_node
            .at_path("String/Cash.img")
            .map(|node| nodes.push(node));
        base_node
            .at_path("String/Consume.img")
            .map(|node| nodes.push(node));
        base_node
            .at_path("String/Eqp.img")
            .map(|node| nodes.push(node));
        base_node
            .at_path("String/Map.img")
            .map(|node| nodes.push(node));
        base_node
            .at_path("String/Mob.img")
            .map(|node| nodes.push(node));
        base_node
            .at_path("String/Npc.img")
            .map(|node| nodes.push(node));
        base_node
            .at_path("String/Pet.img")
            .map(|node| nodes.push(node));
        base_node
            .at_path("String/Skill.img")
            .map(|node| nodes.push(node));
        base_node
            .at_path("String/Ins.img")
            .map(|node| nodes.push(node));

        nodes
    };

    let result = Mutex::new(Vec::new());

    string_nodes.par_iter().for_each(|node| {
        let parse_success = {
            let mut node_write = node.write().unwrap();
            node_write.parse(node).is_ok()
        };

        if parse_success {
            node.read()
                .unwrap()
                .children
                .values()
                .collect::<Vec<_>>()
                .par_iter()
                .for_each(|node| {
                    walk_node(node, false, &|node| {
                        let node_read = node.read().unwrap();
                        /* name are always string node */
                        if node_read.try_as_string().is_some() {
                            let name = string::resolve_string_from_node(&node);
                            if let Ok(name) = name {
                                if &name == item_name {
                                    let mut result = result.lock().unwrap();
                                    /* get actual id */
                                    let id = node_read
                                        .parent
                                        .upgrade()
                                        .unwrap()
                                        .read()
                                        .unwrap()
                                        .name
                                        .clone();
                                    result.push(id);
                                }
                            }
                        }
                    })
                })
        };
    });

    println!("{:?}", result.into_inner().unwrap());

    println!("finding time: {:?}", start.elapsed());
}
