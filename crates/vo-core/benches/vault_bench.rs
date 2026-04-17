use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use std::collections::HashMap;
use vo_core::vault::CredentialVault;
use vo_types::credentials::{
    AccessPolicy, Credential, CredentialId, CredentialKind, CredentialStatus, CredentialVersion,
    CredentialVersionId, Principal, RotationPolicy, RotationState, SecretValue, VaultEntry,
    VaultEntryId,
};
use vo_types::TimestampMs;

fn make_credential_id(i: u32) -> CredentialId {
    let ulid = ulid::Ulid::from((i as u64, 0));
    CredentialId::parse(&ulid.to_string()).unwrap()
}

fn make_version_id(i: u32) -> CredentialVersionId {
    let ulid = ulid::Ulid::from((i as u64 + 1000, 0));
    CredentialVersionId::parse(&ulid.to_string()).unwrap()
}

fn make_vault_entry_id(i: u32) -> VaultEntryId {
    let ulid = ulid::Ulid::from((i as u64 + 2000, 0));
    VaultEntryId::parse(&ulid.to_string()).unwrap()
}

fn make_vault_entry(i: u32) -> VaultEntry {
    let id = make_credential_id(i);
    let version_id = make_version_id(i);
    let secret = SecretValue::new(vec![0u8; 32], [0u8; 12], 1).unwrap();
    let credential = Credential {
        id: id.clone(),
        kind: CredentialKind::ApiKey,
        name: format!("cred-{i}"),
        current_version: version_id.clone(),
        versions: vec![CredentialVersion::new(
            version_id,
            secret,
            CredentialStatus::Active,
            TimestampMs::now(),
            None,
        )],
        rotation_policy: RotationPolicy::Manual,
        metadata: HashMap::new(),
        created_at: TimestampMs::now(),
        updated_at: TimestampMs::now(),
    };
    VaultEntry {
        entry_id: make_vault_entry_id(i),
        credential,
        access_policy: AccessPolicy::new(vec![Principal::System]),
        rotation_state: RotationState::new(),
    }
}

fn bench_vault_create(c: &mut Criterion) {
    let mut group = c.benchmark_group("vault_create");

    for count in [1, 10, 50] {
        group.bench_function(format!("create_{}_credentials", count), |b| {
            b.iter_batched(
                || {
                    (
                        CredentialVault::new(),
                        (0..count).map(make_vault_entry).collect::<Vec<_>>(),
                    )
                },
                |(mut vault, entries)| {
                    for entry in entries {
                        black_box(vault.create_credential(black_box(entry)));
                    }
                },
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

fn bench_vault_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("vault_get");

    let mut vault = CredentialVault::new();
    let ids: Vec<CredentialId> = (0..100)
        .map(|i| {
            let id = vault.create_credential(make_vault_entry(i)).unwrap();
            id
        })
        .collect();

    group.bench_function("get_existing", |b| {
        b.iter(|| black_box(vault.get_credential(black_box(&ids[50]))))
    });
    group.bench_function("get_missing", |b| {
        b.iter(|| black_box(vault.get_credential(black_box(&ids[0]))))
    });

    group.finish();
}

fn bench_vault_list(c: &mut Criterion) {
    let mut group = c.benchmark_group("vault_list");

    for count in [10, 50, 100] {
        let mut vault = CredentialVault::new();
        for i in 0..count {
            vault.create_credential(make_vault_entry(i)).unwrap();
        }
        group.bench_function(format!("list_{}_credentials", count), |b| {
            b.iter(|| black_box(vault.list_credentials()))
        });
    }

    group.finish();
}

fn bench_vault_rotate(c: &mut Criterion) {
    let mut group = c.benchmark_group("vault_rotate");

    let mut vault = CredentialVault::new();
    let ids: Vec<CredentialId> = (0..50)
        .map(|i| vault.create_credential(make_vault_entry(i)).unwrap())
        .collect();

    group.bench_function("rotate_single", |b| {
        b.iter_batched(
            || {
                let mut v = CredentialVault::new();
                for i in 0..50 {
                    v.create_credential(make_vault_entry(i)).unwrap();
                }
                v
            },
            |mut v| black_box(v.rotate(black_box(&ids[25]), black_box(None))),
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_vault_revoke_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("vault_revoke_all");

    let principal = Principal::System;

    for version_count in [1, 5, 10] {
        group.bench_function(format!("revoke_all_{}_versions", version_count), |b| {
            b.iter_batched(
                || {
                    let mut vault = CredentialVault::new();
                    let id = vault.create_credential(make_vault_entry(0)).unwrap();
                    for _ in 1..version_count {
                        vault.rotate(&id, None).unwrap();
                    }
                    (vault, id)
                },
                |(mut vault, id)| {
                    black_box(vault.revoke_all(black_box(&id), black_box(&principal)))
                },
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

fn bench_vault_update_metadata(c: &mut Criterion) {
    let mut group = c.benchmark_group("vault_update_metadata");

    let mut vault = CredentialVault::new();
    let id = vault.create_credential(make_vault_entry(0)).unwrap();

    let metadata: HashMap<String, String> = (0..10)
        .map(|i| (format!("key-{i}"), format!("value-{i}")))
        .collect();

    group.bench_function("update_10_keys", |b| {
        b.iter(|| black_box(vault.update_metadata(black_box(&id), black_box(metadata.clone()))))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_vault_create,
    bench_vault_get,
    bench_vault_list,
    bench_vault_rotate,
    bench_vault_revoke_all,
    bench_vault_update_metadata,
);
criterion_main!(benches);
