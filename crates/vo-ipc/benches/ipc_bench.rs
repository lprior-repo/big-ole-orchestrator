use std::hint::black_box;
use criterion::{criterion_group, criterion_main, Criterion};
use vo_ipc::stderr::{finalize_capture, update_capture, StderrCapture, MAX_STDERR_BYTES};

fn bench_update_capture_small(c: &mut Criterion) {
    let capture = StderrCapture::empty();
    let chunk = b"error line\n";
    c.bench_function("update_capture_small_chunk", |b| {
        b.iter(|| black_box(update_capture(black_box(capture.clone()), black_box(chunk))))
    });
}

fn bench_update_capture_large(c: &mut Criterion) {
    let capture = StderrCapture::empty();
    let chunk: Vec<u8> = (0..8192).map(|b| (b % 255) as u8).collect();
    c.bench_function("update_capture_large_chunk", |b| {
        b.iter(|| {
            black_box(update_capture(
                black_box(capture.clone()),
                black_box(&chunk),
            ))
        })
    });
}

fn bench_update_capture_at_limit(c: &mut Criterion) {
    let capture = StderrCapture {
        bytes: vec![0u8; MAX_STDERR_BYTES],
        truncated: false,
        observed_bytes: MAX_STDERR_BYTES,
    };
    let chunk = b"should be truncated\n";
    c.bench_function("update_capture_at_limit", |b| {
        b.iter(|| black_box(update_capture(black_box(capture.clone()), black_box(chunk))))
    });
}

fn bench_finalize_capture_truncated(c: &mut Criterion) {
    let capture = StderrCapture {
        bytes: vec![0u8; MAX_STDERR_BYTES],
        truncated: true,
        observed_bytes: MAX_STDERR_BYTES + 1024,
    };
    c.bench_function("finalize_capture_truncated", |b| {
        b.iter(|| black_box(finalize_capture(black_box(capture.clone()))))
    });
}

fn bench_finalize_capture_no_truncation(c: &mut Criterion) {
    let capture = StderrCapture {
        bytes: b"normal output\n".to_vec(),
        truncated: false,
        observed_bytes: 14,
    };
    c.bench_function("finalize_capture_no_truncation", |b| {
        b.iter(|| black_box(finalize_capture(black_box(capture.clone()))))
    });
}

criterion_group!(
    benches,
    bench_update_capture_small,
    bench_update_capture_large,
    bench_update_capture_at_limit,
    bench_finalize_capture_truncated,
    bench_finalize_capture_no_truncation,
);
criterion_main!(benches);
