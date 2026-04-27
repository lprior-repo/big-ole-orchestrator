#[cfg(kani)]
mod verification {
    use super::*;
    use crate::types::FenceToken;

    #[kani::proof]
    fn verify_fencetoken_monotonic_advance() {
        let v: u64 = kani::any();
        kani::assume(v > 0);
        kani::assume(v < u64::MAX);
        let token = FenceToken::new(v).unwrap();
        let next = token.next();
        assert!(matches!(next, Ok(value) if value.inner().get() == v + 1));
    }
}