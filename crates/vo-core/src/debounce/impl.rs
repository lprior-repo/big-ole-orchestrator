use super::*;
use std::cmp;

impl Debouncer {
    /// Creates a new debouncer.
    ///
    /// # Errors
    /// Returns `Error::InvalidDebounceDuration` if the duration is 0.
    /// Returns `Error::WatcherChannelClosed` if the event receiver is already closed.
    pub fn new(duration: Duration, mut event_rx: Receiver<FileEvent>) -> Result<Self, Error> {
        if duration.as_nanos() == 0 {
            return Err(Error::InvalidDebounceDuration);
        }

        if tokio::runtime::Handle::try_current().is_err() {
            return Err(Error::NoRuntime);
        }

        let mut initial_event = None;
        match event_rx.try_recv() {
            Err(TryRecvError::Disconnected) => return Err(Error::WatcherChannelClosed),
            Err(TryRecvError::Empty) => {}
            Ok(event) => initial_event = Some(event),
        }

        let (ready_tx, ready_rx) = tokio::sync::mpsc::channel(10_000);

        tokio::spawn(Self::background_task(
            duration,
            event_rx,
            ready_tx,
            initial_event,
        ));

        Ok(Self { duration, ready_rx })
    }

    /// Waits for the next debounced file event.
    ///
    /// # Errors
    /// Returns `Error::WatcherChannelClosed` if the underlying watcher drops and all events are yielded.
    /// Returns `Error::DebouncerInternal` if an internal error occurs (e.g., timer overflow).
    pub async fn next_debounced_event(&mut self) -> Result<PathBuf, Error> {
        self.ready_rx
            .recv()
            .await
            .unwrap_or(Err(Error::WatcherChannelClosed))
    }

    async fn background_task(
        duration: Duration,
        mut event_rx: Receiver<FileEvent>,
        ready_tx: Sender<Result<PathBuf, Error>>,
        initial_event: Option<FileEvent>,
    ) {
        let mut pending: HashMap<PathBuf, Instant> = HashMap::new();
        let mut channel_closed = false;

        if let Some(event) = initial_event {
            if Self::handle_event_or_fail(&mut pending, duration, event, &ready_tx)
                .await
                .is_err()
            {
                return;
            }
        }

        loop {
            if Self::drain_channel(
                &mut event_rx,
                &mut pending,
                duration,
                &ready_tx,
                &mut channel_closed,
            )
            .await
            .is_err()
            {
                return;
            }

            let (expired, next_deadline) = Self::calculate_deadlines(&mut pending);

            for path in expired {
                pending.remove(&path);
                if ready_tx.send(Ok(path)).await.is_err() {
                    return;
                }
            }

            if channel_closed && pending.is_empty() {
                if ready_tx
                    .send(Err(Error::WatcherChannelClosed))
                    .await
                    .is_err()
                {
                    // Receiver closed
                }
                return;
            }

            if Self::wait_for_events(
                &mut event_rx,
                &mut pending,
                duration,
                &ready_tx,
                &mut channel_closed,
                next_deadline,
            )
            .await
            .is_err()
            {
                return;
            }
        }
    }

    fn process_single_event_sync(
        pending: &mut HashMap<PathBuf, Instant>,
        duration: Duration,
        event: FileEvent,
        now: Instant,
    ) -> Result<(), Error> {
        match event {
            FileEvent::Modify(path) => {
                let deadline = now.checked_add(duration).ok_or(Error::DebouncerInternal)?;
                pending.insert(path, deadline);
            }
            FileEvent::Delete(path) => {
                pending.remove(&path);
            }
        }
        Ok(())
    }

    async fn handle_event_or_fail(
        pending: &mut HashMap<PathBuf, Instant>,
        duration: Duration,
        event: FileEvent,
        ready_tx: &Sender<Result<PathBuf, Error>>,
    ) -> Result<(), ()> {
        if Self::process_single_event_sync(pending, duration, event, Instant::now()).is_err() {
            if ready_tx.send(Err(Error::DebouncerInternal)).await.is_err() {
                // Ignore send error
            }
            Err(())
        } else {
            Ok(())
        }
    }

    fn calculate_deadlines(
        pending: &mut HashMap<PathBuf, Instant>,
    ) -> (Vec<PathBuf>, Option<Instant>) {
        let now = Instant::now();
        let mut next_deadline = None;
        let mut expired = Vec::new();

        for (path, &deadline) in pending.iter() {
            if deadline <= now {
                expired.push(path.clone());
            } else {
                next_deadline = Some(match next_deadline {
                    Some(d) => cmp::min(d, deadline),
                    None => deadline,
                });
            }
        }

        expired.sort();
        (expired, next_deadline)
    }

    async fn drain_channel(
        event_rx: &mut Receiver<FileEvent>,
        pending: &mut HashMap<PathBuf, Instant>,
        duration: Duration,
        ready_tx: &Sender<Result<PathBuf, Error>>,
        channel_closed: &mut bool,
    ) -> Result<(), ()> {
        if *channel_closed {
            return Ok(());
        }
        loop {
            match event_rx.try_recv() {
                Ok(event) => {
                    Self::handle_event_or_fail(pending, duration, event, ready_tx).await?;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    *channel_closed = true;
                    break;
                }
            }
        }
        Ok(())
    }

    async fn wait_for_events(
        event_rx: &mut Receiver<FileEvent>,
        pending: &mut HashMap<PathBuf, Instant>,
        duration: Duration,
        ready_tx: &Sender<Result<PathBuf, Error>>,
        channel_closed: &mut bool,
        next_deadline: Option<Instant>,
    ) -> Result<(), ()> {
        let sleep_fut = async {
            if let Some(d) = next_deadline {
                sleep_until(d).await;
            } else {
                std::future::pending::<()>().await;
            }
        };

        tokio::select! {
            () = sleep_fut => {}
            event_opt = event_rx.recv(), if !*channel_closed => {
                match event_opt {
                    Some(event) => {
                        Self::handle_event_or_fail(pending, duration, event, ready_tx).await?;
                    }
                    None => *channel_closed = true,
                }
            }
        }
        Ok(())
    }
}
