//! Serial native file execution isolated from authority and socket polling.

use crate::{controller_io::NativeIoSignal, storage_io::NativeStorage};
use birdman_game_runtime::{ApplicationInput, ApplicationRequest};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::Duration,
};

/// Executes already-routed storage requests on one bounded FIFO worker.
pub(crate) struct NativeStorageWorker {
    requests: Option<mpsc::SyncSender<ApplicationRequest>>,
    completions: Option<mpsc::Receiver<Vec<ApplicationInput>>>,
    stopped: Arc<AtomicBool>,
    done: mpsc::Receiver<()>,
    thread: Option<JoinHandle<()>>,
}

impl NativeStorageWorker {
    pub(crate) fn new(
        mut storage: NativeStorage,
        signal: Arc<NativeIoSignal>,
    ) -> Result<Self, String> {
        let (requests, receiver) = mpsc::sync_channel::<ApplicationRequest>(64);
        let (completed, completions) = mpsc::sync_channel(64);
        let (finished, done) = mpsc::sync_channel(1);
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = stopped.clone();
        let thread = std::thread::Builder::new()
            .name("birdman-storage-io".into())
            .spawn(move || {
                while let Ok(request) = receiver.recv() {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    let inputs = storage
                        .execute(&request)
                        .expect("NativeApplicationIo routes only supported storage requests");
                    if completed.send(inputs).is_err() {
                        break;
                    }
                    signal.notify();
                }
                let _ = finished.send(());
                signal.notify();
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            requests: Some(requests),
            completions: Some(completions),
            stopped,
            done,
            thread: Some(thread),
        })
    }

    pub(crate) fn submit(
        &self,
        request: ApplicationRequest,
    ) -> Result<(), Box<ApplicationRequest>> {
        let Some(sender) = &self.requests else {
            return Err(Box::new(request));
        };
        sender.try_send(request).map_err(|error| match error {
            mpsc::TrySendError::Full(request) | mpsc::TrySendError::Disconnected(request) => {
                Box::new(request)
            }
        })
    }

    pub(crate) fn poll(&mut self) -> Vec<ApplicationInput> {
        self.completions
            .as_ref()
            .map(|receiver| receiver.try_iter().flatten().collect())
            .unwrap_or_default()
    }

    /// Release queues and join only after the worker confirms its terminal state.
    pub(crate) fn shutdown(&mut self) -> Result<(), String> {
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        self.stopped.store(true, Ordering::Release);
        self.requests.take();
        self.completions.take();
        match self.done.recv_timeout(Duration::from_secs(2)) {
            Ok(()) => thread
                .join()
                .map_err(|_| "Native storage worker panicked".to_owned()),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = thread.join();
                Err("Native storage worker terminated before completing its I/O".into())
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                Err("Native storage I/O did not terminate within two seconds".into())
            }
        }
    }
}

impl Drop for NativeStorageWorker {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
