use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use serde_json::{json, Value};

// Helper to generate test data
fn generate_test_data(size: usize) -> Vec<Value> {
    (0..size)
        .map(|i| {
            json!({
                "id": i,
                "name": format!("Item {}", i),
                "status": if i % 3 == 0 { "Running" } else if i % 3 == 1 { "Pending" } else { "Failed" },
                "value": i * 10,
                "created": "2024-01-01T10:00:00Z",
            })
        })
        .collect()
}

// Simulate accessing data via clone (current approach)
fn render_table_with_clone(data: &[Value], visible_rows: usize) -> Vec<String> {
    let cloned_data = data.to_vec();
    cloned_data
        .iter()
        .take(visible_rows)
        .map(|row| format!("{}", row.get("name").unwrap()))
        .collect()
}

// Simulate accessing data via indices (optimized approach)
fn render_table_with_indices(
    data: &[Value],
    indices: &[usize],
    visible_rows: usize,
) -> Vec<String> {
    indices
        .iter()
        .take(visible_rows)
        .map(|&i| format!("{}", data[i].get("name").unwrap()))
        .collect()
}

// Simulate applying conditional styling
fn apply_conditional_style_current(value: &str) -> &'static str {
    // Multiple string comparisons (typical styling logic)
    if value == "Running" {
        "green"
    } else if value == "Pending" {
        "yellow"
    } else if value == "Failed" {
        "red"
    } else {
        "gray"
    }
}

// Simulate applying conditional styling with match (more efficient)
fn apply_conditional_style_optimized(value: &str) -> &'static str {
    match value {
        "Running" => "green",
        "Pending" => "yellow",
        "Failed" => "red",
        _ => "gray",
    }
}

fn bench_table_rendering(c: &mut Criterion) {
    let mut group = c.benchmark_group("table_rendering");
    let sizes = vec![100, 1000, 10000];
    let visible_rows = 50; // Typical terminal height

    for size in sizes {
        let data = generate_test_data(size);
        let indices: Vec<usize> = (0..data.len()).collect();

        // Benchmark with cloning
        group.bench_with_input(BenchmarkId::new("with_clone", size), &size, |b, _| {
            b.iter(|| {
                black_box(render_table_with_clone(&data, visible_rows));
            });
        });

        // Benchmark with indices
        group.bench_with_input(BenchmarkId::new("with_indices", size), &size, |b, _| {
            b.iter(|| {
                black_box(render_table_with_indices(&data, &indices, visible_rows));
            });
        });
    }

    group.finish();
}

fn bench_row_styling(c: &mut Criterion) {
    let mut group = c.benchmark_group("row_styling");
    let data = generate_test_data(1000);
    let statuses: Vec<&str> = data
        .iter()
        .map(|row| row.get("status").unwrap().as_str().unwrap())
        .collect();

    // Benchmark current styling approach
    group.bench_function("current", |b| {
        b.iter(|| {
            let styles: Vec<_> = statuses
                .iter()
                .map(|&status| apply_conditional_style_current(status))
                .collect();
            black_box(styles);
        });
    });

    // Benchmark optimized styling approach
    group.bench_function("optimized", |b| {
        b.iter(|| {
            let styles: Vec<_> = statuses
                .iter()
                .map(|&status| apply_conditional_style_optimized(status))
                .collect();
            black_box(styles);
        });
    });

    group.finish();
}

fn bench_cell_formatting(c: &mut Criterion) {
    let mut group = c.benchmark_group("cell_formatting");
    let data = generate_test_data(1000);
    let visible_rows = 50;

    // Benchmark formatting cells with String allocation
    group.bench_function("with_allocation", |b| {
        b.iter(|| {
            let formatted: Vec<String> = data
                .iter()
                .take(visible_rows)
                .map(|row| {
                    format!(
                        "{} | {} | {}",
                        row.get("name").unwrap(),
                        row.get("status").unwrap(),
                        row.get("value").unwrap()
                    )
                })
                .collect();
            black_box(formatted);
        });
    });

    // Benchmark formatting cells with pre-allocated capacity
    group.bench_function("with_capacity", |b| {
        b.iter(|| {
            let mut formatted = Vec::with_capacity(visible_rows);
            for row in data.iter().take(visible_rows) {
                formatted.push(format!(
                    "{} | {} | {}",
                    row.get("name").unwrap(),
                    row.get("status").unwrap(),
                    row.get("value").unwrap()
                ));
            }
            black_box(formatted);
        });
    });

    group.finish();
}

fn bench_visible_row_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("visible_rows");
    let data = generate_test_data(10000);
    let visible_rows = 50;
    let start_index = 1000;

    // Benchmark extracting visible rows by cloning
    group.bench_function("clone_slice", |b| {
        b.iter(|| {
            let visible = data[start_index..start_index + visible_rows].to_vec();
            black_box(visible);
        });
    });

    // Benchmark extracting visible rows by reference
    group.bench_function("borrow_slice", |b| {
        b.iter(|| {
            let visible = &data[start_index..start_index + visible_rows];
            black_box(visible);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_table_rendering,
    bench_row_styling,
    bench_cell_formatting,
    bench_visible_row_extraction
);
criterion_main!(benches);
