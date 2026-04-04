#[tokio::main]
async fn main() {
    let (tx, rx) = tokio::sync::mpsc::channel::<()>(10);
    drop(tx);
    tracing::info!("closed: {}", rx.is_closed());
}
