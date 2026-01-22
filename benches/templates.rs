use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use serde_json::{json, Value};
use tera::{Context, Tera};

// Helper to create template engine
fn create_tera() -> Tera {
    let mut tera = Tera::default();
    tera.autoescape_on(vec![]);
    tera
}

// Simulate current behavior: clone Tera on every render
fn render_with_clone(template: &str, data: &Value) -> String {
    let tera = create_tera();
    let mut context = Context::new();
    context.insert("value", data);
    context.insert("row", data);

    // Clone tera (current behavior)
    let mut cloned_tera = tera.clone();
    cloned_tera.render_str(template, &context).unwrap()
}

// Simulate optimized behavior: reuse Tera with Arc or caching
fn render_with_reuse(tera: &mut Tera, template: &str, data: &Value) -> String {
    let mut context = Context::new();
    context.insert("value", data);
    context.insert("row", data);

    tera.render_str(template, &context).unwrap()
}

// Simulate template with complex expressions
fn render_complex_template(tera: &mut Tera, iterations: usize) -> Vec<String> {
    let template =
        "{{ value | upper }} - Status: {% if row.status == 'active' %}✓{% else %}✗{% endif %}";

    (0..iterations)
        .map(|i| {
            let data = json!({
                "value": format!("item_{}", i),
                "status": if i % 2 == 0 { "active" } else { "inactive" }
            });
            render_with_reuse(tera, template, &data)
        })
        .collect()
}

fn bench_simple_template_render(c: &mut Criterion) {
    let mut group = c.benchmark_group("template_simple");
    let data = json!({ "name": "Test Item", "value": 42 });
    let simple_template = "{{ row.name }}: {{ row.value }}";

    // Benchmark with cloning (current)
    group.bench_function("with_clone", |b| {
        b.iter(|| {
            black_box(render_with_clone(simple_template, &data));
        });
    });

    // Benchmark with reuse (optimized)
    let mut tera = create_tera();
    group.bench_function("with_reuse", |b| {
        b.iter(|| {
            black_box(render_with_reuse(&mut tera, simple_template, &data));
        });
    });

    group.finish();
}

fn bench_complex_template_render(c: &mut Criterion) {
    let mut group = c.benchmark_group("template_complex");
    let data = json!({
        "name": "Item",
        "status": "active",
        "value": 100,
        "tags": ["tag1", "tag2", "tag3"]
    });

    let complex_template = r#"
        Name: {{ row.name | upper }}
        Status: {% if row.status == "active" %}Active{% else %}Inactive{% endif %}
        Value: {{ row.value * 2 }}
        Tags: {% for tag in row.tags %}{{ tag }}{% if not loop.last %}, {% endif %}{% endfor %}
    "#;

    // Benchmark with cloning
    group.bench_function("with_clone", |b| {
        b.iter(|| {
            black_box(render_with_clone(complex_template, &data));
        });
    });

    // Benchmark with reuse
    let mut tera = create_tera();
    group.bench_function("with_reuse", |b| {
        b.iter(|| {
            black_box(render_with_reuse(&mut tera, complex_template, &data));
        });
    });

    group.finish();
}

fn bench_table_cell_rendering(c: &mut Criterion) {
    let mut group = c.benchmark_group("template_table_cells");
    let mut tera = create_tera();

    // Simulate rendering a table with N rows and 5 columns each needing template rendering
    for row_count in [100, 1000] {
        group.bench_with_input(
            BenchmarkId::new("render_cells", row_count),
            &row_count,
            |b, &count| {
                b.iter(|| {
                    black_box(render_complex_template(&mut tera, count));
                });
            },
        );
    }

    group.finish();
}

fn bench_context_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("template_context");
    let data = json!({
        "name": "Item",
        "id": 123,
        "status": "active",
        "value": 100
    });

    // Benchmark creating new context (current)
    group.bench_function("new_context", |b| {
        b.iter(|| {
            let mut context = Context::new();
            context.insert("value", &data);
            context.insert("row", &data);
            black_box(context);
        });
    });

    // Benchmark reusing context (optimized)
    group.bench_function("reuse_context", |b| {
        let mut context = Context::new();
        b.iter(|| {
            context.insert("value", &data);
            context.insert("row", &data);
            black_box(&context);
        });
    });

    group.finish();
}

fn bench_conditional_styling(c: &mut Criterion) {
    let mut group = c.benchmark_group("template_conditional");
    let mut tera = create_tera();
    let template = "{% if value == 'Running' %}green{% elif value == 'Pending' %}yellow{% else %}red{% endif %}";

    let values = vec!["Running", "Pending", "Failed", "Unknown"];

    // Benchmark evaluating conditions for multiple cells
    group.bench_function("evaluate_conditions", |b| {
        b.iter(|| {
            for value in &values {
                let data = json!({ "value": value });
                black_box(render_with_reuse(&mut tera, template, &data));
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_simple_template_render,
    bench_complex_template_render,
    bench_table_cell_rendering,
    bench_context_creation,
    bench_conditional_styling
);
criterion_main!(benches);
