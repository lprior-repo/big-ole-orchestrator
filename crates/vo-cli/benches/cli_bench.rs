use std::hint::black_box;
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use vo_cli::parse::parse_strict_numeric;

fn bench_parse_strict_numeric(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_strict_numeric");
    group.throughput(Throughput::Elements(1));

    group.bench_function("valid_zero", |b| {
        b.iter(|| black_box(parse_strict_numeric(black_box("0"))))
    });

    group.bench_function("valid_small", |b| {
        b.iter(|| black_box(parse_strict_numeric(black_box("42"))))
    });

    group.bench_function("valid_large", |b| {
        b.iter(|| black_box(parse_strict_numeric(black_box("18446744073709551615"))))
    });

    group.bench_function("invalid_empty", |b| {
        b.iter(|| black_box(parse_strict_numeric(black_box(""))))
    });

    group.bench_function("invalid_plus_sign", |b| {
        b.iter(|| black_box(parse_strict_numeric(black_box("+42"))))
    });

    group.bench_function("invalid_negative", |b| {
        b.iter(|| black_box(parse_strict_numeric(black_box("-1"))))
    });

    group.bench_function("invalid_letters", |b| {
        b.iter(|| black_box(parse_strict_numeric(black_box("abc"))))
    });

    group.finish();
}

fn bench_cli_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("cli_interpret");
    group.throughput(Throughput::Elements(1));

    group.bench_function("parse_check_command", |b| {
        b.iter(|| {
            let args = vec!["vo-cli", "check", "/tmp/test.wf"];
            black_box(vo_cli::cli::interpret_cli_from(black_box(args)))
        })
    });

    group.bench_function("parse_gc_command", |b| {
        b.iter(|| {
            let args = vec!["vo-cli", "gc", "--engine-url", "http://localhost:8080"];
            black_box(vo_cli::cli::interpret_cli_from(black_box(args)))
        })
    });

    group.bench_function("parse_gc_dry_run", |b| {
        b.iter(|| {
            let args = vec![
                "vo-cli",
                "gc",
                "--engine-url",
                "http://localhost:8080",
                "--dry-run",
            ];
            black_box(vo_cli::cli::interpret_cli_from(black_box(args)))
        })
    });

    group.bench_function("parse_init_command", |b| {
        b.iter(|| {
            let args = vec![
                "vo-cli",
                "init",
                "/tmp/project",
                "--engine-url",
                "http://localhost:8080",
                "--storage-path",
                "/tmp/storage",
            ];
            black_box(vo_cli::cli::interpret_cli_from(black_box(args)))
        })
    });

    group.bench_function("parse_doctor_command", |b| {
        b.iter(|| {
            let args = vec!["vo-cli", "doctor", "/tmp/project"];
            black_box(vo_cli::cli::interpret_cli_from(black_box(args)))
        })
    });

    group.bench_function("parse_rebuild_command", |b| {
        b.iter(|| {
            let args = vec![
                "vo-cli",
                "rebuild",
                "/tmp/project",
                "--projection-id",
                "proj-123",
            ];
            black_box(vo_cli::cli::interpret_cli_from(black_box(args)))
        })
    });

    group.bench_function("parse_rebuild_list", |b| {
        b.iter(|| {
            let args = vec!["vo-cli", "rebuild", "/tmp/project", "--list-projections"];
            black_box(vo_cli::cli::interpret_cli_from(black_box(args)))
        })
    });

    group.bench_function("parse_purge_command", |b| {
        b.iter(|| {
            let args = vec!["vo-cli", "purge", "01H5JYV4XHGSR2F8KZ9BWNRFMA"];
            black_box(vo_cli::cli::interpret_cli_from(black_box(args)))
        })
    });

    group.bench_function("parse_invalid_command", |b| {
        b.iter(|| {
            let args = vec!["vo-cli", "nonexistent"];
            black_box(vo_cli::cli::interpret_cli_from(black_box(args)))
        })
    });

    group.bench_function("parse_no_args", |b| {
        b.iter(|| {
            let args: Vec<&str> = vec!["vo-cli"];
            black_box(vo_cli::cli::interpret_cli_from(black_box(args)))
        })
    });

    group.finish();
}

criterion_group!(benches, bench_parse_strict_numeric, bench_cli_parsing,);
criterion_main!(benches);
