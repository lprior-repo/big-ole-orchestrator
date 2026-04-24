#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod fence_token_tests {
    use crate::FenceToken;
    use crate::ParseError;

    #[test]
    fn fence_token_new_rejects_zero() {
        let result = FenceToken::new(0);
        assert_eq!(
            result,
            Err(ParseError::ZeroValue {
                type_name: "FenceToken"
            })
        );
    }

    #[test]
    fn fence_token_new_accepts_one() {
        let token = FenceToken::new(1).expect("1 is valid");
        assert_eq!(token.inner().get(), 1);
    }

    #[test]
    fn fence_token_next_increments() {
        let t1 = FenceToken::new(1).unwrap();
        let t2 = t1.next().unwrap();
        assert_eq!(t2.inner().get(), 2);
        assert!(t2 > t1);
    }

    #[test]
    fn fence_token_next_on_max_returns_error() {
        let max_token = FenceToken::new(u64::MAX).unwrap();
        let result = max_token.next();
        assert!(result.is_err());
    }

    #[test]
    fn fence_token_monotonicity_chain() {
        let mut token = FenceToken::new(1).unwrap();
        for i in 2..100u64 {
            let next = token.next().unwrap();
            assert_eq!(next.inner().get(), i);
            assert!(next > token);
            token = next;
        }
    }

    #[test]
    fn fence_token_ordering() {
        let t1 = FenceToken::new(1).unwrap();
        let t10 = FenceToken::new(10).unwrap();
        assert!(t1 < t10);
        assert!(t10 > t1);
        assert!(t1 != t10);
    }

    #[test]
    fn fence_token_display() {
        let token = FenceToken::new(42).unwrap();
        assert_eq!(format!("{token}"), "42");
    }
}
