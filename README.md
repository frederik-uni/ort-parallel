# Ort Parallel
This crate is just a session pool for ONNX Runtime.
It will load more sessions with the same configuration & packed weights until the pool is full.

## Features
- sync
- async

## Exmaple
Sync
```rs
let builder = SessionBuilder::new()
    .unwrap()
    .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
    .unwrap();
let pool = SessionPool::commit_from_file(builder, Path::new("model.onnx"), 10).unwrap();
pool.run(inputs!{...}).unwrap();
```

Async
```rs
let builder = SessionBuilder::new()
    .unwrap()
    .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
    .unwrap();
let pool = AsyncSessionPool::commit_from_file(builder, Path::new("model.onnx"), 10).unwrap();
pool.load_all().await.unwrap();
pool.run_async(inputs! {...}, &RunOptions::new().unwrap())
    .await
    .unwrap();
```
