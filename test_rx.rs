#[tokio::main]
async fn main() {
    let (tx, rx) = tokio::sync::mpsc::channel::<()>(10);
    drop(tx);
    println!("closed: {}", rx.is_closed());
}
