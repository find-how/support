use store::{
    Store, BatchStore, BatchOperation,
    backends::sled::SledStore,
};
use bytes::Bytes;
use testing::store::{
    property::{TestStoreState, TestBatchOp},
};
use testing::store::property::test_kvs_strategy;
use proptest::prelude::*;
use proptest::test_runner::TestRunner;
use proptest::strategy::ValueTree;

#[tokio::test]
async fn test_basic_operations() -> store::Result<()> {
    let store = SledStore::new_test_store();

    // Test set and get
    store.set(b"key1", Bytes::from("value1")).await?;
    assert_eq!(store.get(b"key1").await?.unwrap(), Bytes::from("value1"));

    // Test delete
    store.delete(b"key1").await?;
    assert!(store.get(b"key1").await?.is_none());

    // Test batch operations
    let kvs = vec![
        (b"batch1".to_vec(), Bytes::from("value1")),
        (b"batch2".to_vec(), Bytes::from("value2")),
    ];
    store.batch_set(kvs.clone()).await?;

    // Test range
    let range = store.range(b"batch1"..b"batch3").await?;
    let range_vec: Vec<_> = range.into_iter().collect();
    assert_eq!(range_vec.len(), 2);

    Ok(())
}

#[tokio::test]
async fn test_batch_operations() -> store::Result<()> {
    let store = SledStore::new_test_store();
    let mut batch = store.batch();

    // Add operations to batch
    batch.set(b"batch1", b"value1")?;
    batch.set(b"batch2", b"value2")?;

    // Execute batch
    store.execute_batch(batch).await?;

    // Verify results
    assert_eq!(store.get(b"batch1").await?.unwrap(), Bytes::from("value1"));
    assert_eq!(store.get(b"batch2").await?.unwrap(), Bytes::from("value2"));

    Ok(())
}

#[tokio::test]
async fn test_property_based() -> store::Result<()> {
    let store = SledStore::new_test_store();
    let mut state = TestStoreState::new();

    // Test basic operations
    let mut runner = TestRunner::default();
    let strategy = test_kvs_strategy(100);
    let tree = strategy.new_tree(&mut runner).unwrap();
    let kvs = tree.current();

    for kv in kvs {
        store.set(&kv.key, Bytes::from(kv.value.clone())).await?;
        state.apply_batch_op(TestBatchOp::Set(kv));
    }

    // Verify state matches
    for (key, value) in state.kvs.iter() {
        let stored = store.get(key).await?;
        assert_eq!(stored.as_ref().map(|b| b.as_ref()), Some(value.as_slice()));
    }

    Ok(())
}
