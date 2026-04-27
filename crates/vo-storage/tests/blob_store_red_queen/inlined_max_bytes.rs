use vo_types::INLINED_MAX_BYTES;

#[test]
fn red_queen_inlined_max_bytes_is_4096() {
    assert_eq!(INLINED_MAX_BYTES, 4096, "INLINED_MAX_BYTES must be 4096");
}