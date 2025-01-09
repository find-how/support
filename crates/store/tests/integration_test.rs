use store::{
    backends::sled::SledStore,
    Store, RangeStore, BatchStore,
};
use testing::{
    store::{
        bench::{bench_store, BenchConfig},
        edge::{run_edge_cases, EdgeCase, EdgeCaseConfig},
        property::{
            test_kv_strategy, test_kvs_strategy, test_batch_ops_strategy, test_range_strategy,
            TestStoreState, TestBatchOp, TestKeyValue,
        },
        stress::{stress_test, StressConfig},
        TestStore, StoreTestContext,
    },
};

use proptest::prelude::*;
use tokio;

impl TestStore for SledStore {
    fn new_test_store() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        SledStore::new(temp_dir.path()).expect("Failed to create store")
    }
}

#[tokio::test]
async fn test_basic_operations() {
    let context = StoreTestContext::<SledStore>::new();
    let store = context.store;

    // Test set and get
    store.set(b"key1", b"value1").await.unwrap();
    let value = store.get(b"key1").await.unwrap();
    assert_eq!(value.as_deref(), Some(b"value1".as_ref()));

    // Test delete
    store.delete(b"key1").await.unwrap();
    let value = store.get(b"key1").await.unwrap();
    assert_eq!(value, None);
}

#[tokio::test]
async fn test_batch_operations() {
    let context = StoreTestContext::<SledStore>::new();
    let store = context.store;

    // Prepare batch operations
    let kvs = vec![
        (b"key1".to_vec(), b"value1".to_vec()),
        (b"key2".to_vec(), b"value2".to_vec()),
        (b"key3".to_vec(), b"value3".to_vec()),
    ];

    // Execute batch set
    store.batch_set(kvs.clone()).await.unwrap();

    // Verify all keys were set
    for (key, value) in kvs {
        let result = store.get(&key).await.unwrap();
        assert_eq!(result.as_deref(), Some(value.as_slice()));
    }

    // Execute batch delete
    let keys: Vec<_> = vec![b"key1".to_vec(), b"key2".to_vec()];
    store.batch_delete(keys.clone()).await.unwrap();

    // Verify keys were deleted
    for key in keys {
        let result = store.get(&key).await.unwrap();
        assert_eq!(result, None);
    }
}

#[tokio::test]
async fn test_range_operations() {
    let context = StoreTestContext::<SledStore>::new();
    let store = context.store;

    // Prepare data
    let kvs = vec![
        (b"a1".to_vec(), b"value1".to_vec()),
        (b"a2".to_vec(), b"value2".to_vec()),
        (b"b1".to_vec(), b"value3".to_vec()),
        (b"b2".to_vec(), b"value4".to_vec()),
    ];

    // Set all key-value pairs
    store.batch_set(kvs).await.unwrap();

    // Test range query
    let range = store.range(b"a".as_ref()..b"b".as_ref()).await.unwrap();
    let keys: Vec<_> = range.map(|r| r.unwrap().0).collect();
    assert_eq!(keys, vec![b"a1".to_vec(), b"a2".to_vec()]);
}

proptest! {
    #[test]
    fn test_property_based_operations(
        kvs in test_kvs_strategy(10)
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let context = StoreTestContext::<SledStore>::new();
            let store = context.store;

            // Set all key-value pairs
            for kv in &kvs {
                store.set(&kv.key, &kv.value).await.unwrap();
            }

            // Verify all key-value pairs
            for kv in &kvs {
                let result = store.get(&kv.key).await.unwrap();
                prop_assert_eq!(result.as_deref(), Some(kv.value.as_slice()));
            }
            Ok(())
        });
    }

    #[test]
    fn test_property_based_batch_operations(
        ops in test_batch_ops_strategy(10)
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let context = StoreTestContext::<SledStore>::new();
            let store = context.store;
            let mut state = testing::property::TestStoreState::new();

            // Apply operations to both store and state
            for op in ops {
                match op {
                    testing::property::TestBatchOp::Set(kv) => {
                        store.set(&kv.key, &kv.value).await.unwrap();
                        state.apply_batch_op(testing::property::TestBatchOp::Set(kv));
                    }
                    testing::property::TestBatchOp::Delete(key) => {
                        store.delete(&key).await.unwrap();
                        state.apply_batch_op(testing::property::TestBatchOp::Delete(key));
                    }
                }
            }

            // Verify final state
            for (key, value) in state.kvs.iter() {
                let result = store.get(key).await.unwrap();
                prop_assert_eq!(result.as_deref(), Some(value.as_slice()));
            }
            Ok(())
        });
    }

    #[test]
    fn test_property_based_range_operations(
        kvs in test_kvs_strategy(10),
        range in test_range_strategy()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let context = StoreTestContext::<SledStore>::new();
            let store = context.store;
            let mut state = testing::property::TestStoreState::new();

            // Set all key-value pairs
            for kv in kvs {
                store.set(&kv.key, &kv.value).await.unwrap();
                state.apply_batch_op(testing::property::TestBatchOp::Set(kv));
            }

            // Query range
            let store_range = store.range(&range.start..&range.end).await.unwrap();
            let store_kvs: Vec<_> = store_range
                .map(|r| r.unwrap())
                .map(|(k, v)| testing::property::TestKeyValue::new(k, v))
                .collect();

            // Get expected range from state
            let state_kvs = state.get_range(&range);

            // Compare results
            prop_assert_eq!(store_kvs.len(), state_kvs.len());
            for (store_kv, state_kv) in store_kvs.iter().zip(state_kvs.iter()) {
                prop_assert_eq!(store_kv.key, state_kv.key);
                prop_assert_eq!(store_kv.value, state_kv.value);
            }
            Ok(())
        });
    }
}

#[tokio::test]
async fn test_edge_cases() {
    let config = EdgeCaseConfig::default();
    let results = run_edge_cases(|| SledStore::new_test_store(), &config).await;
    assert_eq!(results.failed, 0, "Edge cases failed: {:?}", results.failures);
}

#[tokio::test]
async fn test_stress() {
    let config = StressConfig::default();
    let results = stress_test(|| SledStore::new_test_store(), &config).await;
    assert_eq!(results.errors, 0, "Stress test failed with {} errors", results.errors);
}

criterion::criterion_group! {
    name = benches;
    config = criterion::Criterion::default()
        .measurement_time(std::time::Duration::from_secs(10))
        .sample_size(10);
    targets = bench_sled_store
}

fn bench_sled_store(c: &mut criterion::Criterion) {
    let config = BenchConfig::default();
    bench_store(c, "sled_store", || SledStore::new_test_store(), &config);
}

criterion::criterion_main!(benches);
