use pipeline_engine::{EngineConfig, WorkQueue, Worker};

#[test]
fn test_worker_processing() {
    let mut q = WorkQueue::new();
    q.push("item_1".into());
    let w = Worker::new("w-1");
    let res = w.process_next(&mut q);
    assert!(res.is_some());
    assert!(q.is_empty());
}
