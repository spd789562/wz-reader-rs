use criterion::{black_box, criterion_group, Criterion};
use wz_reader::{WzObjectType, WzNode, WzNodeArc, property::WzSubProperty, WzImage, WzNodeCast};
use wz_reader::version::WzMapleVersion;
use std::sync::Arc;

fn create_int_node(num: i32, parent: &WzNodeArc) -> WzNodeArc {
    WzNode::new(&format!("{}", num).into(), num, Some(parent)).into_lock()
}

fn thin_setup() -> (WzNodeArc, String) {
    let root = WzNode::new(&"root".into(), WzObjectType::Property(WzSubProperty::Property), None).into_lock();

    let (_, mut path) = (0..99).fold((Arc::clone(&root), String::from("")), |node, num| {
        let child = create_int_node(num, &node.0);
        node.0.write().unwrap().children.insert(num.to_string().into(), Arc::clone(&child));
        (child, format!("{}/{}", node.1, num))
    });

    path.remove(0);

    (root, path)
}

fn fat_setup() -> (WzNodeArc, String) {
    let root = WzNode::new(&"root".into(), WzObjectType::Property(WzSubProperty::Property), None).into_lock();

    let (_, mut path) = (0..=500).fold((Arc::clone(&root), String::from("")), |node, _| {
        let first = create_int_node(0, &node.0);
        node.0.write().unwrap().children.insert("0".into(), Arc::clone(&first));

        let last = (1..=500).fold(first, |_, num| {
            let child = create_int_node(num, &node.0);
            node.0.write().unwrap().children.insert(num.to_string().into(), Arc::clone(&child));
            child
        });
        (last, format!("{}/{}", node.1, 500))
    });

    path.remove(0);

    (root, path)
}

fn lookup(node: &WzNodeArc, look_path: &str) {
    assert!(node.read().unwrap().at_path(look_path).is_some());
}
fn parse_and_lookup(node: &WzNodeArc, look_path: &str) {
    assert!(node.write().unwrap().parse(node).is_ok());
    assert!(node.read().unwrap().at_path(look_path).is_some());
}
fn direct_lookup(node: &WzImage, look_path: &str) {
    assert!(node.at_path(look_path).is_ok());
}

fn thin_bench(c: &mut Criterion) {
    let (node, path) = thin_setup();
    c.bench_function("thin node lookup", |b| b.iter(|| lookup(black_box(&node), black_box(&path))));
}

fn fat_bench(c: &mut Criterion) {
    let (node, path) = fat_setup();
    c.bench_function("fat node lookup", |b| b.iter(|| lookup(black_box(&node), black_box(&path))));
}

fn parse_and_access_bench(c: &mut Criterion) {
    let node = WzNode::from_wz_file("./benches/benchmarks/test.wz", Some(WzMapleVersion::BMS), Some(123), None).unwrap().into_lock();
    assert!(node.write().unwrap().parse(&node).is_ok());
    let image_node = node.read().unwrap().at("wz_img.img").unwrap();
    c.bench_function("parse and access lookup", |b| b.iter(|| {
        parse_and_lookup(black_box(&image_node), "1/1/1/1/1/1/1/1/1/1/1");
    }));
}

fn access_after_parsing_bench(c: &mut Criterion) {
    let node = WzNode::from_wz_file("./benches/benchmarks/test.wz", Some(WzMapleVersion::BMS), Some(123), None).unwrap().into_lock();
    assert!(node.write().unwrap().parse(&node).is_ok());
    let image_node = node.read().unwrap().at("wz_img.img").unwrap();
    image_node.write().unwrap().parse(&image_node).unwrap();
    c.bench_function("access after parsing lookup", |b| b.iter(|| {
        lookup(black_box(&image_node), "1/1/1/1/1/1/1/1/1/1/1");
    }));
}

fn direct_access_bench(c: &mut Criterion) {
    let node = WzNode::from_wz_file("./benches/benchmarks/test.wz", Some(WzMapleVersion::BMS), Some(123), None).unwrap().into_lock();
    assert!(node.write().unwrap().parse(&node).is_ok());
    let image_node = node.read().unwrap().at("wz_img.img").unwrap();
    let image_node = image_node.read().unwrap();
    let image_node = image_node.try_as_image().unwrap();
    c.bench_function("direct access lookup", |b| b.iter(|| {
        direct_lookup(black_box(&image_node), "1/1/1/1/1/1/1/1/1/1/1");
    }));
}

criterion_group!(benches, thin_bench, fat_bench, parse_and_access_bench, access_after_parsing_bench, direct_access_bench);