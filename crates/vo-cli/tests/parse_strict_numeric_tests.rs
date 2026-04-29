use vo_cli::parse_strict_numeric;

#[test]
fn parse_strict_numeric_accepts_leading_zeros() {
    assert_eq!(parse_strict_numeric("007").unwrap(), 7);
    assert_eq!(parse_strict_numeric("000").unwrap(), 0);
}

#[test]
fn parse_strict_numeric_rejects_float() {
    assert!(parse_strict_numeric("3.14").is_err());
}

#[test]
fn parse_strict_numeric_rejects_binary() {
    assert!(parse_strict_numeric("0b1010").is_err());
}

#[test]
fn parse_strict_numeric_rejects_octal() {
    assert!(parse_strict_numeric("0o777").is_err());
}

#[test]
fn parse_strict_numeric_rejects_alphanumeric() {
    assert!(parse_strict_numeric("12abc34").is_err());
}

#[test]
fn parse_strict_numeric_rejects_tab() {
    assert!(parse_strict_numeric("\t42").is_err());
}

#[test]
fn parse_strict_numeric_rejects_newline() {
    assert!(parse_strict_numeric("42\n").is_err());
}

#[test]
fn parse_strict_numeric_u64_boundary_minus_one() {
    assert_eq!(
        parse_strict_numeric("18446744073709551614").unwrap(),
        u64::MAX - 1
    );
}
