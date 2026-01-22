use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use serde_json::{json, Value};
use std::fmt::Write;

// Helper function to generate test data
fn generate_test_data(size: usize) -> Vec<Value> {
    (0..size)
        .map(|i| {
            json!({
                "id": i,
                "name": format!("Item {}", i),
                "status": if i % 3 == 0 { "active" } else { "inactive" },
                "description": format!("This is a longer description for item number {} with additional searchable content", i),
                "tags": vec!["tag1", "tag2", "tag3"],
                "metadata": {
                    "created": "2024-01-01",
                    "updated": "2024-01-15",
                    "author": format!("User {}", i % 10)
                }
            })
        })
        .collect()
}

// Current implementation: Uses multiple String allocations and joins
fn item_to_searchable_text_current(item: &Value) -> String {
    fn collect_values(val: &Value) -> Vec<String> {
        match val {
            Value::String(s) => vec![s.clone()],
            Value::Number(n) => vec![n.to_string()],
            Value::Bool(b) => vec![b.to_string()],
            Value::Array(arr) => arr.iter().flat_map(collect_values).collect(),
            Value::Object(map) => map.values().flat_map(collect_values).collect(),
            Value::Null => vec![],
        }
    }

    collect_values(item).join(" ")
}

// Optimized implementation: Uses single buffer with fmt::Write
fn item_to_searchable_text_optimized(item: &Value) -> String {
    let mut buffer = String::with_capacity(256);

    fn collect_values(val: &Value, buffer: &mut String) {
        match val {
            Value::String(s) => {
                if !buffer.is_empty() {
                    buffer.push(' ');
                }
                buffer.push_str(s);
            }
            Value::Number(n) => {
                if !buffer.is_empty() {
                    buffer.push(' ');
                }
                write!(buffer, "{}", n).unwrap();
            }
            Value::Bool(b) => {
                if !buffer.is_empty() {
                    buffer.push(' ');
                }
                write!(buffer, "{}", b).unwrap();
            }
            Value::Array(arr) => {
                for item in arr {
                    collect_values(item, buffer);
                }
            }
            Value::Object(map) => {
                for value in map.values() {
                    collect_values(value, buffer);
                }
            }
            Value::Null => {}
        }
    }

    collect_values(item, &mut buffer);
    buffer
}

// Search with current implementation
fn search_current(data: &[Value], query: &str) -> Vec<usize> {
    let query_lower = query.to_lowercase();
    data.iter()
        .enumerate()
        .filter(|(_, item)| {
            let text = item_to_searchable_text_current(item);
            text.to_lowercase().contains(&query_lower)
        })
        .map(|(i, _)| i)
        .collect()
}

// Search with optimized implementation
fn search_optimized(data: &[Value], query: &str) -> Vec<usize> {
    let query_lower = query.to_lowercase();
    data.iter()
        .enumerate()
        .filter(|(_, item)| {
            let text = item_to_searchable_text_optimized(item);
            text.to_lowercase().contains(&query_lower)
        })
        .map(|(i, _)| i)
        .collect()
}

fn bench_searchable_text_conversion(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_text_conversion");
    let item = json!({
        "id": 123,
        "name": "Test Item",
        "status": "active",
        "description": "A longer description with multiple words to search through",
        "tags": ["tag1", "tag2", "tag3"],
        "metadata": {
            "created": "2024-01-01",
            "author": "User Name"
        }
    });

    // Benchmark current implementation
    group.bench_function("current", |b| {
        b.iter(|| {
            black_box(item_to_searchable_text_current(&item));
        });
    });

    // Benchmark optimized implementation
    group.bench_function("optimized", |b| {
        b.iter(|| {
            black_box(item_to_searchable_text_optimized(&item));
        });
    });

    group.finish();
}

fn bench_full_search(c: &mut Criterion) {
    let sizes = vec![100, 1000, 10000];
    let mut group = c.benchmark_group("search_full");

    for size in sizes {
        let data = generate_test_data(size);

        // Benchmark current search
        group.bench_with_input(BenchmarkId::new("current", size), &size, |b, _| {
            b.iter(|| {
                black_box(search_current(&data, "item 50"));
            });
        });

        // Benchmark optimized search
        group.bench_with_input(BenchmarkId::new("optimized", size), &size, |b, _| {
            b.iter(|| {
                black_box(search_optimized(&data, "item 50"));
            });
        });
    }

    group.finish();
}

fn bench_search_no_match(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_no_match");
    let data = generate_test_data(1000);
    let query = "nonexistent_query_string";

    // Benchmark current - worst case (no matches, searches entire dataset)
    group.bench_function("current", |b| {
        b.iter(|| {
            black_box(search_current(&data, query));
        });
    });

    // Benchmark optimized - worst case
    group.bench_function("optimized", |b| {
        b.iter(|| {
            black_box(search_optimized(&data, query));
        });
    });

    group.finish();
}

fn bench_search_case_sensitivity(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_case");
    let data = generate_test_data(1000);

    // Search with mixed case query
    group.bench_function("mixed_case_current", |b| {
        b.iter(|| {
            black_box(search_current(&data, "ItEm 50"));
        });
    });

    group.bench_function("mixed_case_optimized", |b| {
        b.iter(|| {
            black_box(search_optimized(&data, "ItEm 50"));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_searchable_text_conversion,
    bench_full_search,
    bench_search_no_match,
    bench_search_case_sensitivity
);
criterion_main!(benches);
