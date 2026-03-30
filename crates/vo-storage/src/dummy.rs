fn main() {
    println!(
        "{:?}",
        vo_types::InstanceId::parse("00000000000000000000000001")
    );
    println!(
        "{:?}",
        vo_types::InstanceId::parse("7ZZZZZZZZZZZZZZZZZZZZZZZZZ")
    );
}
