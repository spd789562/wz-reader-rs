use criterion::criterion_main;

mod benchmarks;

criterion_main! {
    benchmarks::node_lookup::benches,
    benchmarks::node_parsing::benches
}