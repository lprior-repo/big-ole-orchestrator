use sha2::Digest;
use vo_storage::blob_store::{BlobRecord, ContentAddress};
use vo_types::{
    BlobFailureAction, BlobRef, BlobStatus, OutputPolicy, OutputRef, INLINED_MAX_BYTES,
};

pub const VALID_SHA256: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
pub const VALID_SHA256_2: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

pub fn make_content_addr() -> ContentAddress {
    ContentAddress::new(VALID_SHA256).unwrap()
}

pub fn make_blob_ref() -> BlobRef {
    BlobRef::new(
        "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
        1024,
        "abcdef0123456789abcdef0123456789",
    )
    .unwrap()
}

pub fn make_blob_record(ref_count: u64) -> BlobRecord {
    let content_addr = make_content_addr();
    BlobRecord::new(content_addr, 1024, ref_count, 1000, Some(2000)).unwrap()
}