use std::hint::black_box;
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use vo_linter::rules::check_random_in_workflow;

fn bench_lint_clean_file(c: &mut Criterion) {
    let mut group = c.benchmark_group("linter_clean");
    group.throughput(Throughput::Elements(1));

    let src = r#"
        fn workflow() {
            let x = ctx.random_u64();
            let y = some_deterministic_fn();
            let z = compute(x, y);
            if z > 100 {
                do_thing_a();
            } else {
                do_thing_b();
            }
        }
        fn helper(a: u64, b: u64) -> u64 {
            a + b
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();

    group.bench_function("check_random_clean_10_lines", |b| {
        b.iter(|| black_box(check_random_in_workflow(black_box(&file))))
    });

    group.finish();
}

fn bench_lint_with_violations(c: &mut Criterion) {
    let mut group = c.benchmark_group("linter_violations");
    group.throughput(Throughput::Elements(1));

    let src = r#"
        fn workflow() {
            let id = Uuid::new_v4();
            let val = rand::random();
            let x = ctx.random_u64();
            let id2 = Uuid::new_v4();
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();

    group.bench_function("check_random_3_violations", |b| {
        b.iter(|| black_box(check_random_in_workflow(black_box(&file))))
    });

    group.finish();
}

fn bench_lint_large_file(c: &mut Criterion) {
    let mut group = c.benchmark_group("linter_large");
    group.throughput(Throughput::Elements(1));

    let mut src = String::from("mod my_module {\n");
    for i in 0..100 {
        src.push_str(&format!(
            "    fn workflow_{}() {{ let x = ctx.random_u64(); let y = helper(x); }}\n",
            i
        ));
    }
    src.push_str("}\n");
    let file: syn::File = syn::parse_str(&src).unwrap();

    group.bench_function("check_random_100_functions_clean", |b| {
        b.iter(|| black_box(check_random_in_workflow(black_box(&file))))
    });

    let mut src_dirty = String::from("mod dirty_module {\n");
    for i in 0..100 {
        if i % 10 == 0 {
            src_dirty.push_str(&format!(
                "    fn workflow_{}() {{ let id = Uuid::new_v4(); let y = helper(id); }}\n",
                i
            ));
        } else {
            src_dirty.push_str(&format!(
                "    fn workflow_{}() {{ let x = ctx.random_u64(); let y = helper(x); }}\n",
                i
            ));
        }
    }
    src_dirty.push_str("}\n");
    let file_dirty: syn::File = syn::parse_str(&src_dirty).unwrap();

    group.bench_function("check_random_100_functions_10_violations", |b| {
        b.iter(|| black_box(check_random_in_workflow(black_box(&file_dirty))))
    });

    group.finish();
}

fn bench_lint_parse_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("linter_parse");

    let src = r#"
        fn workflow() {
            let x = ctx.random_u64();
            let y = some_fn(x);
        }
    "#;

    group.bench_function("parse_str_small", |b| {
        b.iter(|| black_box(syn::parse_str::<syn::File>(black_box(src))))
    });

    let large_src = format!(
        "mod large {{\n{}\n}}\n",
        (0..100)
            .map(|i| format!("    fn f{i}() {{ let x = ctx.random_u64(); let y = helper(x); }}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    group.bench_function("parse_str_100_functions", |b| {
        b.iter(|| black_box(syn::parse_str::<syn::File>(black_box(&large_src))))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_lint_clean_file,
    bench_lint_with_violations,
    bench_lint_large_file,
    bench_lint_parse_overhead,
);
criterion_main!(benches);
