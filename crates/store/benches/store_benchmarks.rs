use criterion::{criterion_group, criterion_main, Criterion};
use store::backends::sled::SledStore;
use testing::{
    bench::{bench_store, BenchConfig},
    TestStore,
};

impl TestStore for SledStore {
    fn new_test_store() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        SledStore::new(temp_dir.path()).expect("Failed to create store")
    }
}

fn bench_sled_store(c: &mut Criterion) {
    let config = BenchConfig {
        operations: 10_000,
        key_size: 16,
        value_size: 100,
        batch_size: 100,
        range_size: 100,
        measurement_time: std::time::Duration::from_secs(10),
        sample_size: 50,
    };

    bench_store(c, "sled_store", || SledStore::new_test_store(), &config);
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(std::time::Duration::from_secs(10))
        .sample_size(50);
    targets = bench_sled_store
}

criterion_main!(benches);
