//! Criterion benchmarks for P2P sync latency
//!
//! Run with: cargo bench --package integration_tests
//!
//! These benchmarks measure:
//! - Handshake latency (Hello -> Welcome)
//! - Sync round-trip (SyncOffer -> SyncAck)
//! - Message throughput at various batch sizes

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

use integration_tests::scenario::MockScenario;

/// Benchmark handshake latency using mock transport
fn bench_handshake_mock(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("handshake_owner_to_node_mock", |b| {
        b.to_async(&rt).iter(|| async {
            let mut scenario = MockScenario::new_mock(true, true, 0).await.unwrap();
            scenario.connect_owner_to_node_mock().await.unwrap();
            scenario.shutdown().await;
        });
    });
}

/// Benchmark space setup (create space + page)
fn bench_space_setup(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("setup_owner_space_page", |b| {
        b.to_async(&rt).iter(|| async {
            let scenario = MockScenario::new_mock(true, false, 0).await.unwrap();
            let _space_info = scenario.setup_owner("Benchmark Space").await.unwrap();
        });
    });
}

/// Benchmark full publish flow (handshake + publish)
fn bench_publish_flow(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("full_publish_flow_mock", |b| {
        b.to_async(&rt).iter(|| async {
            let mut scenario = MockScenario::new_mock(true, true, 0).await.unwrap();

            // Setup owner with space/page
            let space_info = scenario.setup_owner("Benchmark Space").await.unwrap();

            // Connect and publish
            scenario.connect_owner_to_node_mock().await.unwrap();
            scenario.publish_space(&space_info.id).await.unwrap();

            // Wait for publish to complete
            tokio::time::sleep(Duration::from_millis(50)).await;

            scenario.shutdown().await;
        });
    });
}

/// Benchmark viewer sync at different viewer counts
fn bench_viewer_sync(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("viewer_sync");
    group.sample_size(20); // Fewer samples for slower benchmarks

    for viewer_count in [1, 2, 3].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(viewer_count),
            viewer_count,
            |b, &count| {
                b.to_async(&rt).iter(|| async move {
                    let mut scenario = MockScenario::new_mock(true, true, count).await.unwrap();

                    // Setup owner with space/page
                    let space_info = scenario.setup_owner("Benchmark Space").await.unwrap();

                    // Connect owner to node
                    scenario.connect_owner_to_node_mock().await.unwrap();
                    scenario.publish_space(&space_info.id).await.unwrap();

                    // Wait for publish
                    tokio::time::sleep(Duration::from_millis(100)).await;

                    // Get viewer link
                    let viewer_link = scenario.get_viewer_link(&space_info.id).await.unwrap();

                    // Connect all viewers
                    for i in 0..count {
                        scenario
                            .add_viewer_mock(i, &viewer_link, &space_info.page_id)
                            .await
                            .unwrap();
                    }

                    scenario.shutdown().await;
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_handshake_mock,
    bench_space_setup,
    bench_publish_flow,
    bench_viewer_sync,
);
criterion_main!(benches);
