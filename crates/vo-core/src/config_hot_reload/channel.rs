use crate::debounce::FileEvent as DebouncedFileEvent;

use super::error::Error;

pub struct EventChannel {
    tx: tokio::sync::mpsc::Sender<DebouncedFileEvent>,
}

impl EventChannel {
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = tokio::sync::mpsc::channel(capacity);
        Self { tx }
    }

    pub async fn send(&self, event: DebouncedFileEvent) -> Result<(), Error> {
        self.tx
            .send(event)
            .await
            .map_err(|_| Error::EventQueueClosed)
    }

    pub fn sender(&self) -> tokio::sync::mpsc::Sender<DebouncedFileEvent> {
        self.tx.clone()
    }
}
