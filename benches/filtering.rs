use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use serde_json::{json, Value};

// Helper function to generate synthetic test data
fn generate_test_data(size: usize) -> Vec<Value> {
    (0..size)
        .map(|i| {
            json!({
                "id": i,
                "name": format!("Item {}", i),
                "status": if i % 3 == 0 { "active" } else { "inactive" },
                "value": i * 10,
                "description": format!("This is item number {} with some additional text", i),
            })
        })
        .collect()
}

// Simulate current cloning behavior
fn filter_with_clone(data: &[Value], query: &str) -> Vec<Value> {
    // Clone the entire dataset first (current behavior)
    let mut cloned_data = data.to_vec();

    // Filter the cloned data
    cloned_data
        .into_iter()
        .filter(|item| {
            item.to_string()
                .to_lowercase()
                .contains(&query.to_lowercase())
        })
        .collect()
}

// Simulate index-based filtering (proposed optimization)
fn filter_with_indices(data: &[Value], query: &str) -> Vec<usize> {
    (0..data.len())
        .filter(|&i| {
            data[i]
                .to_string()
                .to_lowercase()
                .contains(&query.to_lowercase())
        })
        .collect()
}

// Simulate sorting with cloning (current behavior)
fn sort_with_clone(data: &[Value], column: &str) -> Vec<Value> {
    let mut sorted = data.to_vec();
    sorted.sort_by(|a, b| {
        let a_val = a.get(column).and_then(|v| v.as_str()).unwrap_or("");
        let b_val = b.get(column).and_then(|v| v.as_str()).unwrap_or("");
        a_val.cmp(b_val)
    });
    sorted
}

// Simulate sorting with indices (proposed optimization)
fn sort_with_indices(data: &[Value], column: &str) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..data.len()).collect();
    indices.sort_by(|&a, &b| {
        let a_val = data[a].get(column).and_then(|v| v.as_str()).unwrap_or("");
        let b_val = data[b].get(column).and_then(|v| v.as_str()).unwrap_or("");
        a_val.cmp(b_val)
    });
    indices
}

fn bench_filtering(c: &mut Criterion) {
    let sizes = vec![100, 1000, 10000];
    let mut group = c.benchmark_group("filtering");

    for size in sizes {
        let data = generate_test_data(size);

        // Benchmark cloning-based filter
        group.bench_with_input(BenchmarkId::new("filter_clone", size), &size, |b, _| {
            b.iter(|| {
                black_box(filter_with_clone(&data, "item 50"));
            });
        });

        // Benchmark index-based filter
        group.bench_with_input(BenchmarkId::new("filter_indices", size), &size, |b, _| {
            b.iter(|| {
                black_box(filter_with_indices(&data, "item 50"));
            });
        });
    }

    group.finish();
}

fn bench_sorting(c: &mut Criterion) {
    let sizes = vec![100, 1000, 10000];
    let mut group = c.benchmark_group("sorting");

    for size in sizes {
        let data = generate_test_data(size);

        // Benchmark cloning-based sort
        group.bench_with_input(BenchmarkId::new("sort_clone", size), &size, |b, _| {
            b.iter(|| {
                black_box(sort_with_clone(&data, "name"));
            });
        });

        // Benchmark index-based sort
        group.bench_with_input(BenchmarkId::new("sort_indices", size), &size, |b, _| {
            b.iter(|| {
                black_box(sort_with_indices(&data, "name"));
            });
        });
    }

    group.finish();
}

fn bench_combined_filter_sort(c: &mut Criterion) {
    let sizes = vec![1000, 10000];
    let mut group = c.benchmark_group("filter_and_sort");

    for size in sizes {
        let data = generate_test_data(size);

        // Benchmark cloning-based filter + sort
        group.bench_with_input(BenchmarkId::new("combined_clone", size), &size, |b, _| {
            b.iter(|| {
                let filtered = filter_with_clone(&data, "active");
                black_box(sort_with_clone(&filtered, "name"));
            });
        });

        // Benchmark index-based filter + sort
        group.bench_with_input(BenchmarkId::new("combined_indices", size), &size, |b, _| {
            b.iter(|| {
                let mut indices = filter_with_indices(&data, "active");
                indices.sort_by(|&a, &b| {
                    let a_val = data[a].get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let b_val = data[b].get("name").and_then(|v| v.as_str()).unwrap_or("");
                    a_val.cmp(b_val)
                });
                black_box(indices);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_filtering,
    bench_sorting,
    bench_combined_filter_sort
);
criterion_main!(benches);
