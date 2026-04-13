use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use vo_storage::crypto::{
    decrypt_blob, encrypt_blob, generate_dek, unwrap_dek, wrap_dek, KEK_SIZE_BYTES,
};
use vo_storage::merkle_tree::MerkleTree;

fn bench_key_wrap_unwrap(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto_key_wrap");
    group.throughput(Throughput::Elements(1));

    let dek = generate_dek().unwrap();
    let kek = [0x42u8; KEK_SIZE_BYTES];

    group.bench_function("wrap_dek", |b| {
        b.iter(|| black_box(wrap_dek(black_box(&dek), black_box(&kek))))
    });

    let wrapped = wrap_dek(&dek, &kek).unwrap();
    group.bench_function("unwrap_dek", |b| {
        b.iter(|| black_box(unwrap_dek(black_box(&wrapped), black_box(&kek))))
    });

    group.finish();
}

fn bench_encrypt_decrypt_blob(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto_blob");

    let dek = generate_dek().unwrap();

    let small_data = b"hello world payload";
    group.throughput(Throughput::Bytes(small_data.len() as u64));
    group.bench_function("encrypt_small_19b", |b| {
        b.iter(|| black_box(encrypt_blob(black_box(small_data), black_box(&dek))))
    });

    let encrypted_small = encrypt_blob(small_data, &dek).unwrap();
    group.bench_function("decrypt_small_19b", |b| {
        b.iter(|| black_box(decrypt_blob(black_box(&encrypted_small), black_box(&dek))))
    });

    let medium_data: Vec<u8> = (0..4096).map(|b| (b % 255) as u8).collect();
    group.throughput(Throughput::Bytes(medium_data.len() as u64));
    group.bench_function("encrypt_medium_4kb", |b| {
        b.iter(|| black_box(encrypt_blob(black_box(&medium_data), black_box(&dek))))
    });

    let encrypted_medium = encrypt_blob(&medium_data, &dek).unwrap();
    group.bench_function("decrypt_medium_4kb", |b| {
        b.iter(|| black_box(decrypt_blob(black_box(&encrypted_medium), black_box(&dek))))
    });

    let large_data: Vec<u8> = (0..65536).map(|b| (b % 255) as u8).collect();
    group.throughput(Throughput::Bytes(large_data.len() as u64));
    group.bench_function("encrypt_large_64kb", |b| {
        b.iter(|| black_box(encrypt_blob(black_box(&large_data), black_box(&dek))))
    });

    let encrypted_large = encrypt_blob(&large_data, &dek).unwrap();
    group.bench_function("decrypt_large_64kb", |b| {
        b.iter(|| black_box(decrypt_blob(black_box(&encrypted_large), black_box(&dek))))
    });

    group.finish();
}

fn bench_merkle_tree(c: &mut Criterion) {
    let mut group = c.benchmark_group("merkle_tree");

    let data_1kb: Vec<u8> = (0..1024).map(|b| (b % 255) as u8).collect();
    group.throughput(Throughput::Bytes(data_1kb.len() as u64));
    group.bench_function("build_1kb_chunk_256", |b| {
        b.iter(|| black_box(MerkleTree::new(black_box(&data_1kb), black_box(256))))
    });

    let data_64kb: Vec<u8> = (0..65536).map(|b| (b % 255) as u8).collect();
    group.throughput(Throughput::Bytes(data_64kb.len() as u64));
    group.bench_function("build_64kb_chunk_4k", |b| {
        b.iter(|| black_box(MerkleTree::new(black_box(&data_64kb), black_box(4096))))
    });

    let data_1mb: Vec<u8> = (0..1_048_576).map(|b| (b % 255) as u8).collect();
    group.throughput(Throughput::Bytes(data_1mb.len() as u64));
    group.bench_function("build_1mb_chunk_64k", |b| {
        b.iter(|| black_box(MerkleTree::new(black_box(&data_1mb), black_box(65536))))
    });

    group.finish();
}

fn bench_merkle_proof(c: &mut Criterion) {
    let mut group = c.benchmark_group("merkle_proof");
    group.throughput(Throughput::Elements(1));

    let data: Vec<u8> = (0..65536).map(|b| (b % 255) as u8).collect();
    let tree = MerkleTree::new(&data, 4096);

    group.bench_function("generate_proof_leaf_0", |b| {
        b.iter(|| black_box(tree.proof(black_box(0))))
    });

    group.bench_function("generate_proof_leaf_mid", |b| {
        b.iter(|| black_box(tree.proof(black_box(tree.leaf_hashes.len() / 2))))
    });

    let proof = tree.proof(0).unwrap();
    let root = tree.root_hash();
    group.bench_function("verify_proof", |b| {
        b.iter(|| black_box(proof.verify(black_box(root))))
    });

    group.bench_function("verify_proof_wrong_root", |b| {
        b.iter(|| black_box(proof.verify(black_box([0xFFu8; 32]))))
    });

    group.finish();
}

fn bench_merkle_chunk_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("merkle_chunk_size");
    let data: Vec<u8> = (0..1_048_576).map(|b| (b % 255) as u8).collect();

    for chunk_size in [512, 1024, 4096, 16384, 65536] {
        let label = format!("build_1mb_chunk_{chunk_size}");
        group.throughput(Throughput::Bytes(data.len() as u64));
        group.bench_function(&label, |b| {
            b.iter(|| {
                black_box(MerkleTree::new(
                    black_box(&data),
                    black_box(chunk_size as u64),
                ))
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_key_wrap_unwrap,
    bench_encrypt_decrypt_blob,
    bench_merkle_tree,
    bench_merkle_proof,
    bench_merkle_chunk_sizes,
);
criterion_main!(benches);
