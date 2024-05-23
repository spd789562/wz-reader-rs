use criterion::{criterion_group, Criterion};
use wz_reader::{version::WzMapleVersion, WzNode};

fn bench(c: &mut Criterion) {
    c.bench_function("node parsing", |b| {
        b.iter(|| {
            let node = WzNode::from_wz_file_full(
                "./benches/benchmarks/test.wz",
                Some(WzMapleVersion::BMS),
                Some(123),
                None,
                None,
            )
            .unwrap()
            .into_lock();
            assert!(node.write().unwrap().parse(&node).is_ok());
        })
    });
}

criterion_group!(benches, bench);
