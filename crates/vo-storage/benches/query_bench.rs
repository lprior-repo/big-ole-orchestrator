use std::hint::black_box;
use criterion::{criterion_group, criterion_main, Criterion};
use vo_storage::instance_index::{decode_instance_index_key, encode_instance_index_key};
use vo_storage::key_encoding::{
    decode_dedupe_key, decode_effect_key, decode_event_key, decode_instance_id, decode_lease_key,
    decode_length_prefixed, decode_sequence_number, decode_step_id, decode_timer_key,
    decode_u16_be, decode_u64_be, encode_dedupe_key, encode_effect_key, encode_event_key,
    encode_instance_id, encode_instance_index_key_for_status, encode_lease_key,
    encode_length_prefixed, encode_sequence_number, encode_step_id, encode_timer_key,
    encode_u16_be, encode_u64_be, get_dedupe_key_prefix, get_event_key_prefix,
    get_lease_key_prefix_for_instance, get_timer_key_prefix_for_time,
};
use vo_storage::query::{
    decode_key, encode_key, epoch_prefix_generator, error_mapper, lineage_prefix_generator,
    prefix_generator, IteratorState, LineageQuery,
};
use vo_types::{Epoch, InstanceId, InstanceStatus, SequenceNumber, StepId, TimestampMs};

fn bench_encode_key(c: &mut Criterion) {
    c.bench_function("encode_key_u64_max", |b| {
        b.iter(|| black_box(encode_key(black_box(u64::MAX))))
    });

    c.bench_function("encode_key_sequence_1", |b| {
        b.iter(|| black_box(encode_key(black_box(1u64))))
    });

    c.bench_function("encode_key_sequence_1000", |b| {
        b.iter(|| black_box(encode_key(black_box(1000u64))))
    });
}

fn bench_decode_key(c: &mut Criterion) {
    let bytes = u64::MAX.to_be_bytes();
    c.bench_function("decode_key_u64_max", |b| {
        b.iter(|| black_box(decode_key(black_box(&bytes))))
    });

    let seq_bytes = 1u64.to_be_bytes();
    c.bench_function("decode_key_sequence_1", |b| {
        b.iter(|| black_box(decode_key(black_box(&seq_bytes))))
    });

    let thousand_bytes = 1000u64.to_be_bytes();
    c.bench_function("decode_key_sequence_1000", |b| {
        b.iter(|| black_box(decode_key(black_box(&thousand_bytes))))
    });
}

fn bench_prefix_generator(c: &mut Criterion) {
    let id = InstanceId::from_bytes([0x42; 16]);
    c.bench_function("prefix_generator_ulid", |b| {
        b.iter(|| black_box(prefix_generator(black_box(&id))))
    });

    let id2 = InstanceId::from_bytes([0x01; 16]);
    c.bench_function("prefix_generator_various_bytes", |b| {
        b.iter(|| black_box(prefix_generator(black_box(&id2))))
    });
}

fn bench_lineage_prefix_generator(c: &mut Criterion) {
    c.bench_function("lineage_prefix_generator_short", |b| {
        b.iter(|| black_box(lineage_prefix_generator(black_box("wf-123"))))
    });

    c.bench_function("lineage_prefix_generator_medium", |b| {
        b.iter(|| black_box(lineage_prefix_generator(black_box("wf-lineage-abc-123"))))
    });

    let long_lineage = "x".repeat(200);
    c.bench_function("lineage_prefix_generator_long", |b| {
        b.iter(|| black_box(lineage_prefix_generator(black_box(&long_lineage))))
    });
}

fn bench_epoch_prefix_generator(c: &mut Criterion) {
    c.bench_function("epoch_prefix_generator_epoch_0", |b| {
        b.iter(|| {
            black_box(epoch_prefix_generator(
                black_box("wf-123"),
                black_box(Epoch::ZERO),
            ))
        })
    });

    c.bench_function("epoch_prefix_generator_epoch_100", |b| {
        b.iter(|| {
            black_box(epoch_prefix_generator(
                black_box("wf-123"),
                black_box(Epoch::new(100)),
            ))
        })
    });

    c.bench_function("epoch_prefix_generator_epoch_max", |b| {
        b.iter(|| {
            black_box(epoch_prefix_generator(
                black_box("wf-123"),
                black_box(Epoch::new(u64::MAX)),
            ))
        })
    });
}

fn bench_lineage_query_to_prefix(c: &mut Criterion) {
    let id = InstanceId::from_bytes([0x42; 16]);
    let query_instance = LineageQuery::InstanceId(&id);
    c.bench_function("lineage_query_instance_id_to_prefix", |b| {
        b.iter(|| black_box(query_instance.to_prefix()))
    });

    let query_lineage = LineageQuery::LineageWide {
        lineage_id: "wf-abc-123",
    };
    c.bench_function("lineage_query_lineage_wide_to_prefix", |b| {
        b.iter(|| black_box(query_lineage.to_prefix()))
    });

    let query_epoch = LineageQuery::EpochSpecific {
        lineage_id: "wf-xyz",
        epoch: Epoch::new(5),
    };
    c.bench_function("lineage_query_epoch_specific_to_prefix", |b| {
        b.iter(|| black_box(query_epoch.to_prefix()))
    });
}

fn bench_iterator_state(c: &mut Criterion) {
    c.bench_function("iterator_state_advance_first", |b| {
        let mut state = IteratorState::new();
        b.iter(|| {
            state = IteratorState::new();
            black_box(state.advance(black_box(1), black_box(make_envelope(1))))
        })
    });

    c.bench_function("iterator_state_advance_consecutive", |b| {
        let mut state = IteratorState::new();
        state.advance(1, make_envelope(1));
        b.iter(|| black_box(state.advance(black_box(2), black_box(make_envelope(2)))))
    });

    c.bench_function("iterator_state_advance_gap_detection", |b| {
        let mut state = IteratorState::new();
        state.advance(1, make_envelope(1));
        b.iter(|| black_box(state.advance(black_box(3), black_box(make_envelope(3)))))
    });
}

fn bench_encode_decode_roundtrip(c: &mut Criterion) {
    c.bench_function("encode_decode_roundtrip_1", |b| {
        b.iter(|| {
            let encoded = encode_key(1u64).unwrap();
            black_box(decode_key(&encoded)).unwrap()
        })
    });

    c.bench_function("encode_decode_roundtrip_1000", |b| {
        b.iter(|| {
            let encoded = encode_key(1000u64).unwrap();
            black_box(decode_key(&encoded)).unwrap()
        })
    });

    c.bench_function("encode_decode_roundtrip_max", |b| {
        b.iter(|| {
            let encoded = encode_key(u64::MAX).unwrap();
            black_box(decode_key(&encoded)).unwrap()
        })
    });
}

fn bench_prefix_roundtrip(c: &mut Criterion) {
    let id = InstanceId::from_bytes([0x42; 16]);
    c.bench_function("prefix_generator_roundtrip", |b| {
        b.iter(|| {
            let prefix = prefix_generator(&id).unwrap();
            black_box(prefix.len())
        })
    });

    c.bench_function("lineage_prefix_roundtrip", |b| {
        b.iter(|| {
            let prefix = lineage_prefix_generator("wf-lineage-123").unwrap();
            black_box(prefix.len())
        })
    });

    c.bench_function("epoch_prefix_roundtrip", |b| {
        b.iter(|| {
            let prefix = epoch_prefix_generator("wf-123", Epoch::new(42)).unwrap();
            black_box(prefix.len())
        })
    });
}

fn make_envelope(seq: u64) -> vo_types::EventEnvelope {
    vo_types::EventEnvelope {
        schema_version: 1,
        instance_id: "test-instance".to_string(),
        sequence: seq,
        timestamp_ms: 1000,
        payload: serde_json::json!({"type": "WorkflowStarted", "workflow_id": "wf-1"}),
        metadata: vo_types::events::EventMetadata::default(),
    }
}

fn bench_key_encoding_u64(c: &mut Criterion) {
    c.bench_function("encode_u64_be_max", |b| {
        b.iter(|| black_box(encode_u64_be(black_box(u64::MAX))))
    });
    c.bench_function("encode_u64_be_one", |b| {
        b.iter(|| black_box(encode_u64_be(black_box(1u64))))
    });
    c.bench_function("encode_u64_be_thousand", |b| {
        b.iter(|| black_box(encode_u64_be(black_box(1000u64))))
    });

    let bytes = u64::MAX.to_be_bytes();
    c.bench_function("decode_u64_be_max", |b| {
        b.iter(|| black_box(decode_u64_be(black_box(&bytes))).unwrap())
    });
    let one_bytes = 1u64.to_be_bytes();
    c.bench_function("decode_u64_be_one", |b| {
        b.iter(|| black_box(decode_u64_be(black_box(&one_bytes))).unwrap())
    });
}

fn bench_key_encoding_u16(c: &mut Criterion) {
    c.bench_function("encode_u16_be_max", |b| {
        b.iter(|| black_box(encode_u16_be(black_box(u16::MAX))))
    });
    c.bench_function("encode_u16_be_one", |b| {
        b.iter(|| black_box(encode_u16_be(black_box(1u16))))
    });

    let bytes = u16::MAX.to_be_bytes();
    c.bench_function("decode_u16_be_max", |b| {
        b.iter(|| black_box(decode_u16_be(black_box(&bytes))).unwrap())
    });
}

fn bench_key_encoding_instance_id(c: &mut Criterion) {
    let id = InstanceId::from_bytes([0x42; 16]);
    c.bench_function("encode_instance_id", |b| {
        b.iter(|| black_box(encode_instance_id(black_box(&id))).unwrap())
    });

    let id_bytes = [0x42u8; 16];
    c.bench_function("decode_instance_id", |b| {
        b.iter(|| black_box(decode_instance_id(black_box(&id_bytes))).unwrap())
    });
}

fn bench_key_encoding_step_id(c: &mut Criterion) {
    let step_id = StepId::parse("step-abc-123").unwrap();
    c.bench_function("encode_step_id", |b| {
        b.iter(|| black_box(encode_step_id(black_box(&step_id))))
    });

    let encoded = encode_step_id(&step_id);
    c.bench_function("decode_step_id", |b| {
        b.iter(|| black_box(decode_step_id(black_box(&encoded))).unwrap())
    });
}

fn bench_key_encoding_sequence_number(c: &mut Criterion) {
    let seq = SequenceNumber::try_from(1000u64).unwrap();
    c.bench_function("encode_sequence_number", |b| {
        b.iter(|| black_box(encode_sequence_number(black_box(seq))))
    });

    let _bytes = 1000u64.to_be_bytes();
    c.bench_function("sequence_number_roundtrip", |b| {
        b.iter(|| {
            let encoded =
                encode_sequence_number(SequenceNumber::try_from(black_box(1000u64)).unwrap());
            let decoded = u64::from_be_bytes(encoded);
            black_box(SequenceNumber::try_from(decoded).unwrap())
        })
    });
}

fn bench_key_encoding_event_key(c: &mut Criterion) {
    let id = InstanceId::from_bytes([0x42; 16]);
    let seq = SequenceNumber::try_from(1000u64).unwrap();
    c.bench_function("encode_event_key", |b| {
        b.iter(|| black_box(encode_event_key(black_box(&id), black_box(seq))))
    });

    let key = encode_event_key(&id, seq);
    c.bench_function("decode_event_key", |b| {
        b.iter(|| black_box(decode_event_key(black_box(&key))).unwrap())
    });
}

fn bench_key_encoding_timer_key(c: &mut Criterion) {
    let id = InstanceId::from_bytes([0x42; 16]);
    c.bench_function("encode_timer_key", |b| {
        b.iter(|| black_box(encode_timer_key(black_box(1000u64), black_box(&id))))
    });

    let key = encode_timer_key(1000u64, &id);
    c.bench_function("decode_timer_key", |b| {
        b.iter(|| black_box(decode_timer_key(black_box(&key))).unwrap())
    });
}

fn bench_key_encoding_lease_key(c: &mut Criterion) {
    let id = InstanceId::from_bytes([0x42; 16]);
    let step_id = StepId::parse("step-abc-123").unwrap();
    c.bench_function("encode_lease_key", |b| {
        b.iter(|| black_box(encode_lease_key(black_box(&id), black_box(&step_id))))
    });

    let key = encode_lease_key(&id, &step_id);
    c.bench_function("decode_lease_key", |b| {
        b.iter(|| black_box(decode_lease_key(black_box(&key))).unwrap())
    });
}

fn bench_key_encoding_dedupe_key(c: &mut Criterion) {
    c.bench_function("encode_dedupe_key_short", |b| {
        b.iter(|| black_box(encode_dedupe_key(black_box("key-123"))))
    });
    c.bench_function("encode_dedupe_key_long", |b| {
        b.iter(|| black_box(encode_dedupe_key(black_box("key-with-a-much-longer-idempotency-key-that-is-200-chars-long-to-test-length-prefixed-encoding-performance"))))
    });

    let encoded = encode_dedupe_key("key-123");
    c.bench_function("decode_dedupe_key", |b| {
        b.iter(|| black_box(decode_dedupe_key(black_box(&encoded))).unwrap())
    });
}

fn bench_key_encoding_effect_key(c: &mut Criterion) {
    let id = InstanceId::from_bytes([0x42; 16]);
    let seq = SequenceNumber::try_from(1000u64).unwrap();
    c.bench_function("encode_effect_key", |b| {
        b.iter(|| black_box(encode_effect_key(black_box(&id), black_box(seq))))
    });
}

fn bench_key_encoding_instance_index_key(c: &mut Criterion) {
    let id = InstanceId::from_bytes([0x42; 16]);
    c.bench_function("encode_instance_index_key_for_status", |b| {
        b.iter(|| {
            black_box(encode_instance_index_key_for_status(
                black_box(InstanceStatus::Running as u8),
                black_box(TimestampMs::try_from(1000u64).unwrap().as_u64()),
                black_box(&id),
            ))
        })
    });

    let key = encode_instance_index_key_for_status(
        InstanceStatus::Running as u8,
        TimestampMs::try_from(1000u64).unwrap().as_u64(),
        &id,
    );
    c.bench_function("decode_instance_index_key", |b| {
        b.iter(|| black_box(decode_instance_index_key(black_box(&key))).unwrap())
    });
}

fn bench_instance_index_encoding(c: &mut Criterion) {
    let id = InstanceId::from_bytes([0x42; 16]);
    c.bench_function("instance_index_encode_key", |b| {
        b.iter(|| {
            black_box(encode_instance_index_key(
                black_box(InstanceStatus::Running),
                black_box(TimestampMs::try_from(1000u64).unwrap()),
                black_box(&id),
            ))
        })
    });

    let key = encode_instance_index_key(
        InstanceStatus::Running,
        TimestampMs::try_from(1000u64).unwrap(),
        &id,
    )
    .unwrap();
    c.bench_function("instance_index_decode_key", |b| {
        b.iter(|| black_box(decode_instance_index_key(black_box(&key))).unwrap())
    });
}

fn bench_key_encoding_prefixes(c: &mut Criterion) {
    let id = InstanceId::from_bytes([0x42; 16]);
    c.bench_function("get_event_key_prefix", |b| {
        b.iter(|| black_box(get_event_key_prefix(black_box(&id))))
    });

    c.bench_function("get_timer_key_prefix_for_time", |b| {
        b.iter(|| black_box(get_timer_key_prefix_for_time(black_box(1000u64))))
    });

    c.bench_function("get_lease_key_prefix_for_instance", |b| {
        b.iter(|| black_box(get_lease_key_prefix_for_instance(black_box(&id))))
    });

    c.bench_function("get_dedupe_key_prefix", |b| {
        b.iter(|| black_box(get_dedupe_key_prefix(black_box("key-123"))))
    });
}

fn bench_length_prefixed(c: &mut Criterion) {
    let short_bytes = b"short";
    let long_bytes: Vec<u8> = (0..1000u16).map(|i| i as u8).collect::<Vec<u8>>();

    c.bench_function("encode_length_prefixed_short", |b| {
        b.iter(|| black_box(encode_length_prefixed(black_box(short_bytes))))
    });
    c.bench_function("encode_length_prefixed_long", |b| {
        b.iter(|| black_box(encode_length_prefixed(black_box(&long_bytes))))
    });

    let encoded_short = encode_length_prefixed(short_bytes);
    c.bench_function("decode_length_prefixed_roundtrip_short", |b| {
        b.iter(|| {
            let len = u16::from_be_bytes([encoded_short[0], encoded_short[1]]);
            black_box(len as usize)
        })
    });

    let encoded_long = encode_length_prefixed(&long_bytes);
    c.bench_function("decode_length_prefixed_roundtrip_long", |b| {
        b.iter(|| {
            let len = u16::from_be_bytes([encoded_long[0], encoded_long[1]]);
            black_box(len as usize)
        })
    });
}

fn bench_decode_length_prefixed(c: &mut Criterion) {
    let short_encoded = encode_length_prefixed(b"short");
    c.bench_function("decode_length_prefixed_short", |b| {
        b.iter(|| black_box(decode_length_prefixed(black_box(&short_encoded))).unwrap())
    });

    let long_bytes: Vec<u8> = (0..1000u16).map(|i| i as u8).collect::<Vec<u8>>();
    let long_encoded = encode_length_prefixed(&long_bytes);
    c.bench_function("decode_length_prefixed_long", |b| {
        b.iter(|| black_box(decode_length_prefixed(black_box(&long_encoded))).unwrap())
    });
}

fn bench_decode_sequence_number(c: &mut Criterion) {
    let encoded = encode_sequence_number(SequenceNumber::try_from(1000u64).unwrap());
    c.bench_function("decode_sequence_number", |b| {
        b.iter(|| black_box(decode_sequence_number(black_box(&encoded))).unwrap())
    });
}

fn bench_decode_event_key(c: &mut Criterion) {
    let id = InstanceId::from_bytes([0x42; 16]);
    let seq = SequenceNumber::try_from(1000u64).unwrap();
    let key = encode_event_key(&id, seq);
    c.bench_function("decode_event_key", |b| {
        b.iter(|| black_box(decode_event_key(black_box(&key))).unwrap())
    });
}

fn bench_decode_timer_key(c: &mut Criterion) {
    let id = InstanceId::from_bytes([0x42; 16]);
    let key = encode_timer_key(1000u64, &id);
    c.bench_function("decode_timer_key", |b| {
        b.iter(|| black_box(decode_timer_key(black_box(&key))).unwrap())
    });
}

fn bench_decode_lease_key(c: &mut Criterion) {
    let id = InstanceId::from_bytes([0x42; 16]);
    let step_id = StepId::parse("step-abc-123").unwrap();
    let key = encode_lease_key(&id, &step_id);
    c.bench_function("decode_lease_key", |b| {
        b.iter(|| black_box(decode_lease_key(black_box(&key))).unwrap())
    });
}

fn bench_decode_effect_key(c: &mut Criterion) {
    let id = InstanceId::from_bytes([0x42; 16]);
    let seq = SequenceNumber::try_from(1000u64).unwrap();
    let key = encode_effect_key(&id, seq);
    c.bench_function("decode_effect_key", |b| {
        b.iter(|| black_box(decode_effect_key(black_box(&key))).unwrap())
    });
}

fn bench_error_mapper(c: &mut Criterion) {
    let unsupported_err = vo_types::events::Error::UnsupportedEnvelopeVersion(99);
    c.bench_function("error_mapper_unsupported_version", |b| {
        b.iter(|| black_box(error_mapper(black_box(&unsupported_err))))
    });

    let invalid_err = vo_types::events::Error::InvalidInput;
    c.bench_function("error_mapper_invalid_input", |b| {
        b.iter(|| black_box(error_mapper(black_box(&invalid_err))))
    });

    let corrupt_err = vo_types::events::Error::InvalidEnvelopeFormat;
    c.bench_function("error_mapper_corrupt_payload", |b| {
        b.iter(|| black_box(error_mapper(black_box(&corrupt_err))))
    });
}

criterion_group!(
    benches,
    bench_encode_key,
    bench_decode_key,
    bench_prefix_generator,
    bench_lineage_prefix_generator,
    bench_epoch_prefix_generator,
    bench_lineage_query_to_prefix,
    bench_iterator_state,
    bench_encode_decode_roundtrip,
    bench_prefix_roundtrip,
    bench_key_encoding_u64,
    bench_key_encoding_u16,
    bench_key_encoding_instance_id,
    bench_key_encoding_step_id,
    bench_key_encoding_sequence_number,
    bench_key_encoding_event_key,
    bench_key_encoding_timer_key,
    bench_key_encoding_lease_key,
    bench_key_encoding_dedupe_key,
    bench_key_encoding_effect_key,
    bench_key_encoding_instance_index_key,
    bench_instance_index_encoding,
    bench_key_encoding_prefixes,
    bench_length_prefixed,
    bench_decode_length_prefixed,
    bench_decode_sequence_number,
    bench_decode_event_key,
    bench_decode_timer_key,
    bench_decode_lease_key,
    bench_decode_effect_key,
    bench_error_mapper,
);
criterion_main!(benches);
