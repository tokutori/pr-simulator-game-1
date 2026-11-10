//! Bounded native execution of Runtime-owned pure asset preparation.

use crate::controller_io::NativeIoSignal;
use birdman_game_runtime::{ApplicationAssetPreparationResult, ApplicationAssetPreparationWork};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(crate) struct NativeAssetPreparationWorker {
    commands: Option<mpsc::SyncSender<ApplicationAssetPreparationWork>>,
    results: mpsc::Receiver<ApplicationAssetPreparationResult>,
    finished: mpsc::Receiver<()>,
    thread: Option<JoinHandle<()>>,
    stopping: Arc<AtomicBool>,
    occupied: bool,
}

impl NativeAssetPreparationWorker {
    pub(crate) fn new(signal: Arc<NativeIoSignal>) -> Result<Self, String> {
        let (commands, receiver) = mpsc::sync_channel::<ApplicationAssetPreparationWork>(1);
        let (output, results) = mpsc::sync_channel(1);
        let (done, finished) = mpsc::sync_channel(1);
        let stopping = Arc::new(AtomicBool::new(false));
        let stop = stopping.clone();
        let thread = thread::Builder::new()
            .name("birdman-asset-preparation".into())
            .spawn(move || {
                while let Ok(work) = receiver.recv() {
                    let job_id = work.id();
                    let started = Instant::now();
                    let result = work.execute(&stop);
                    let elapsed = started.elapsed();
                    if elapsed > Duration::from_millis(100) {
                        eprintln!(
                            "Host asset CPU job {job_id}: {:.1} ms",
                            elapsed.as_secs_f64() * 1000.0
                        );
                    }
                    if output.send(result).is_err() {
                        break;
                    }
                    signal.notify();
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                }
                let _ = done.send(());
                signal.notify();
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            commands: Some(commands),
            results,
            finished,
            thread: Some(thread),
            stopping,
            occupied: false,
        })
    }

    pub(crate) fn is_idle(&self) -> bool {
        !self.occupied && self.commands.is_some() && !self.stopping.load(Ordering::Acquire)
    }

    pub(crate) fn submit(
        &mut self,
        work: ApplicationAssetPreparationWork,
    ) -> Result<(), Box<ApplicationAssetPreparationWork>> {
        if self.occupied {
            return Err(Box::new(work));
        }
        let Some(commands) = &self.commands else {
            return Err(Box::new(work));
        };
        commands.try_send(work).map_err(|error| match error {
            mpsc::TrySendError::Full(work) | mpsc::TrySendError::Disconnected(work) => {
                Box::new(work)
            }
        })?;
        self.occupied = true;
        Ok(())
    }

    pub(crate) fn poll(&mut self) -> Result<Option<ApplicationAssetPreparationResult>, String> {
        match self.results.try_recv() {
            Ok(result) => {
                self.occupied = false;
                Ok(Some(result))
            }
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected)
                if self.thread.is_some() && !self.stopping.load(Ordering::Acquire) =>
            {
                Err("Native asset preparation worker terminated".into())
            }
            Err(mpsc::TryRecvError::Disconnected) => Ok(None),
        }
    }

    pub(crate) fn stop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        self.commands.take();
    }

    pub(crate) fn shutdown(&mut self) -> Result<(), String> {
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        self.stop();
        match self.finished.recv_timeout(Duration::from_secs(2)) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => thread
                .join()
                .map_err(|_| "Native asset preparation worker panicked".to_owned()),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                drop(thread);
                Err("Native asset preparation did not stop within two seconds".into())
            }
        }
    }
}

impl Drop for NativeAssetPreparationWorker {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
