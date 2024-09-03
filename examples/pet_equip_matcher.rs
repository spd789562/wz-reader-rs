use rayon::iter::*;
use std::sync::Mutex;
use wz_reader::property::string;
use wz_reader::util::resolve_base;
use wz_reader::WzNodeCast;

// usage:
//   cargo run --example pet_equip_matcher -- "path/to/Base.wz" "pet_id"
//   cargo run --example pet_equip_matcher -- "D:\Path\To\Base.wz" "5000042"
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let base_path = args.get(1).expect("missing base path");
    let target_pet_id = args.get(2).expect("missing target pet id");
    let base_node = resolve_base(&base_path, None).unwrap();

    let start = std::time::Instant::now();

    let pet_equip = base_node
        .at_path("Character/PetEquip")
        .expect("pet equip path not found");

    let childrens = pet_equip.children.read().unwrap();

    let pet_equip_items = childrens.values().collect::<Vec<_>>();

    let result_items: Mutex<Vec<String>> = Mutex::new(Vec::new());

    pet_equip_items.par_iter().for_each(|node| {
        if let Some(image) = node.try_as_image() {
            if image.at_path(target_pet_id).is_ok() {
                let name = node.name.clone().replace(".img", "");

                if &name == "01802000" {
                    return;
                }

                let mut result_items = result_items.lock().unwrap();

                let striped_name = name.strip_prefix('0').map(|striped| striped.to_string());

                result_items.push(striped_name.unwrap_or(name));
            }
        }
    });

    let mut ids = result_items.into_inner().unwrap();

    ids.sort();

    let string_node = base_node
        .at_path("String/Eqp.img")
        .expect("string node not found");

    let string_img_node = string_node
        .try_as_image()
        .expect("string node is not wz image");

    let pet_equip_node = string_img_node
        .at_path("Eqp/PetEquip")
        .expect("pet equip node not found");

    let names: Vec<_> = ids
        .par_iter()
        .map(|id| {
            let pet_equip_item = pet_equip_node
                .at(id)
                .expect(format!("pet equip item {} not found", id).as_str());
            let name = pet_equip_item.at("name").expect("name not found");
            string::resolve_string_from_node(&name)
        })
        .collect();

    println!("{:?}", names);

    println!("finding time: {:?}", start.elapsed());
}
