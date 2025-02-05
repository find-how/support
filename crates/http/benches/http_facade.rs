use criterion::{black_box, criterion_group, criterion_main, Criterion};
use http::{HttpFacade, Request, Response};
use tokio::runtime::Runtime;

fn bench_mock_response(c: &mut Criterion) {
    let facade = HttpFacade::new();
    let rt = Runtime::new().unwrap();

    c.bench_function("mock_url", |b| {
        b.iter(|| {
            facade.mock_url(black_box("/test"))
        })
    });

    c.bench_function("mock_pattern", |b| {
        b.iter(|| {
            facade.mock_pattern(black_box(r"/users/\d+")).unwrap()
        })
    });

    c.bench_function("mock_response_with", |b| {
        b.iter(|| {
            rt.block_on(async {
                facade.mock_url("/test").respond_with(Response {
                    status: 200,
                    headers: vec![],
                    body: "test".into(),
                }).await
            })
        })
    });
}

fn bench_request_matching(c: &mut Criterion) {
    let facade = HttpFacade::new();
    let rt = Runtime::new().unwrap();

    // Set up test data
    rt.block_on(async {
        facade.mock_pattern(r"/users/\d+")
            .unwrap()
            .respond_with_json(200, &serde_json::json!({"id": 1}))
            .await;
    });

    let request = Request {
        method: "GET".into(),
        url: "/users/123".into(),
        headers: vec![],
        body: None,
    };

    c.bench_function("match_request", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mocks = facade.mocks.read().await;
                black_box(mocks.iter().any(|mock| mock.matcher.matches(&request)))
            })
        })
    });
}

criterion_group!(benches, bench_mock_response, bench_request_matching);
criterion_main!(benches);
