use super::*;
use crate::types::FenceToken;
use std::num::NonZeroU64;

#[test]
fn fencetoken_returns_success_when_value_is_strictly_positive() {
    let ft = FenceToken::new(5).unwrap();
    assert_eq!(ft.inner().get(), 5);
}

#[test]
fn fencetoken_returns_success_when_value_is_minimum_valid_limit() {
    let ft = FenceToken::new(1).unwrap();
    assert_eq!(ft.inner().get(), 1);
}

#[test]
fn fencetoken_returns_success_when_value_is_maximum_valid_input_scenario() {
    let ft = FenceToken::new(u64::MAX).unwrap();
    assert_eq!(ft.inner().get(), u64::MAX);
}

#[test]
fn fencetoken_returns_success_when_value_is_large_typical_value() {
    let ft = FenceToken::new(u64::MAX - 1).unwrap();
    assert_eq!(ft.inner().get(), u64::MAX - 1);
}

#[test]
fn fencetoken_returns_zero_value_error_when_value_is_zero() {
    let res = FenceToken::new(0);
    assert_eq!(
        res,
        Err(crate::ParseError::ZeroValue {
            type_name: "FenceToken"
        })
    );
}

#[test]
fn fencetoken_returns_success_when_parsed_from_typical_string() {
    let ft = FenceToken::parse("42").unwrap();
    assert_eq!(ft.inner().get(), 42);
}

#[test]
fn fencetoken_returns_success_when_parsed_from_minimum_boundary_scenario() {
    let ft = FenceToken::parse("1").unwrap();
    assert_eq!(ft.inner().get(), 1);
}

#[test]
fn fencetoken_returns_success_when_parsed_from_maximum_boundary_scenario() {
    let ft = FenceToken::parse("18446744073709551615").unwrap();
    assert_eq!(ft.inner().get(), u64::MAX);
}

#[test]
fn fencetoken_returns_not_an_integer_error_when_parsed_from_empty_string_scenario() {
    let res = FenceToken::parse("");
    assert_eq!(
        res,
        Err(crate::ParseError::NotAnInteger {
            type_name: "FenceToken",
            input: "".to_string()
        })
    );
}

#[test]
fn fencetoken_returns_zero_value_error_when_parsed_from_zero_string() {
    let res = FenceToken::parse("0");
    assert_eq!(
        res,
        Err(crate::ParseError::ZeroValue {
            type_name: "FenceToken"
        })
    );
}

#[test]
fn fencetoken_returns_not_an_integer_error_when_parsed_from_negative_number() {
    let res = FenceToken::parse("-1");
    assert_eq!(
        res,
        Err(crate::ParseError::NotAnInteger {
            type_name: "FenceToken",
            input: "-1".to_string()
        })
    );
}

#[test]
fn fencetoken_returns_not_an_integer_error_when_parsed_from_float() {
    let res = FenceToken::parse("42.5");
    assert_eq!(
        res,
        Err(crate::ParseError::NotAnInteger {
            type_name: "FenceToken",
            input: "42.5".to_string()
        })
    );
}

#[test]
fn fencetoken_returns_not_an_integer_error_when_parsed_from_alpha_string() {
    let res = FenceToken::parse("abc");
    assert_eq!(
        res,
        Err(crate::ParseError::NotAnInteger {
            type_name: "FenceToken",
            input: "abc".to_string()
        })
    );
}

#[test]
fn fencetoken_returns_not_an_integer_error_when_parsed_from_overflow_behavior() {
    let res = FenceToken::parse("18446744073709551616");
    assert_eq!(
        res,
        Err(crate::ParseError::NotAnInteger {
            type_name: "FenceToken",
            input: "18446744073709551616".to_string()
        })
    );
}

#[test]
fn fencetoken_returns_not_an_integer_error_when_parsed_from_whitespace_string() {
    let res = FenceToken::parse(" 42 ");
    assert_eq!(
        res,
        Err(crate::ParseError::NotAnInteger {
            type_name: "FenceToken",
            input: " 42 ".to_string()
        })
    );
}

#[test]
fn fencetoken_returns_incremented_value_when_next_called_on_minimum_valid_limit() {
    let ft = FenceToken::new(1).unwrap();
    let next = ft.next();
    assert_eq!(next.map(|token| token.inner().get()), Ok(2));
}

#[test]
fn fencetoken_returns_incremented_value_when_next_called_on_typical_value() {
    let ft = FenceToken::new(42).unwrap();
    let next = ft.next();
    assert_eq!(next.map(|token| token.inner().get()), Ok(43));
}

#[test]
fn fencetoken_returns_incremented_value_when_next_called_on_large_value() {
    let ft = FenceToken::new(u64::MAX - 2).unwrap();
    let next = ft.next();
    assert_eq!(next.map(|token| token.inner().get()), Ok(u64::MAX - 1));
}

#[test]
fn fencetoken_returns_success_when_next_called_multiple_times() {
    let ft = FenceToken::new(1).unwrap();
    let next = ft.next().and_then(|token| token.next());
    assert_eq!(next.map(|token| token.inner().get()), Ok(3));
}

#[test]
fn fencetoken_returns_out_of_range_when_next_called_on_u64_max() {
    let ft = FenceToken::new(u64::MAX).unwrap();

    assert_eq!(
        ft.next(),
        Err(crate::ParseError::OutOfRange {
            type_name: "FenceToken",
            value: u64::MAX,
            min: 1,
            max: u64::MAX - 1,
        })
    );
}

#[test]
fn fencetoken_returns_exact_nonzero_when_inner_called_on_minimum_valid_limit() {
    let ft = FenceToken::new(1).unwrap();
    assert_eq!(ft.inner(), NonZeroU64::new(1).unwrap());
}

#[test]
fn fencetoken_returns_exact_nonzero_when_inner_called_on_typical_value() {
    let ft = FenceToken::new(42).unwrap();
    assert_eq!(ft.inner(), NonZeroU64::new(42).unwrap());
}

#[test]
fn fencetoken_returns_exact_nonzero_when_inner_called_on_maximum_valid_limit() {
    let ft = FenceToken::new(u64::MAX).unwrap();
    assert_eq!(ft.inner(), NonZeroU64::new(u64::MAX).unwrap());
}