use crate::helpers::make_blob_record;
use vo_storage::blob_store::{decode_blob_record, encode_blob_record, decode_content_address, encode_content_address};
use crate::helpers::make_content_addr;

#[test]
fn red_queen_content_address_encode_decode_roundtrip() {
    let addr = make_content_addr();
    let encoded = encode_content_address(&addr);
    let decoded = decode_content_address(&encoded).unwrap();
    assert_eq!(addr, decoded);
}

#[test]
fn red_queen_blob_record_encode_decode_roundtrip() {
    let record = make_blob_record(5);
    let encoded = encode_blob_record(&record).unwrap();
    let decoded = decode_blob_record(&encoded).unwrap();
    assert_eq!(record.content_addr(), decoded.content_addr());
    assert_eq!(record.size_bytes(), decoded.size_bytes());
    assert_eq!(record.reference_count(), decoded.reference_count());
}