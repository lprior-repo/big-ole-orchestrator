fn main() {
    println!("{}", serde_json::from_value::<veloxide_vo_types::types::State>(serde_json::json!({ "version": 2 })).unwrap_err());
}
