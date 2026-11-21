//! A long job, such as an analysis of a held capture, that runs on a worker thread that the host supplies. The job reports its
//! progress, stops early when the host cancels it, and can publish results while it works.

use crate::stack::ConfigError;
use crate::storage::{StorageError, StorePath};
use crate::Results;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

/// A long job: it gets the control of its run and returns its final results, or why it failed. A job that sees
/// [`JobControl::cancelled`] returns soon after, with what it has.
pub type Job = Box<dyn FnOnce(&JobControl) -> Result<Results, JobError> + Send>;

/// Why a job failed.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum JobError {
    /// The job has nothing to work on (no held capture).
    #[error("the job has no input")]
    NoInput,
    /// The input or a setting does not suit the job.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// A file of the job cannot be read or written.
    #[error("{path}: {source}")]
    Storage {
        /// The file.
        path: StorePath,
        /// What went wrong.
        source: StorageError,
    },
}

type Publish = Box<dyn Fn(&Results) + Send + Sync>;

/// What a running job uses to talk to its host: its progress, the cancel request, and results published before the end.
/// Cloning shares the same run.
#[derive(Clone)]
pub struct JobControl {
    progress: Arc<AtomicU32>,
    cancel: Arc<AtomicBool>,
    publish: Arc<Publish>,
}

impl std::fmt::Debug for JobControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobControl")
            .field("progress", &self.progress())
            .field("cancelled", &self.cancelled())
            .finish()
    }
}

impl Default for JobControl {
    fn default() -> Self {
        JobControl::new(|_| {})
    }
}

impl JobControl {
    /// A control whose published results go to `publish`. The host makes one for each job.
    ///
    /// ```
    /// use sdrkit_core::JobControl;
    /// let ctl = JobControl::new(|_| {});
    /// ctl.set_progress(0.25);
    /// assert_eq!(ctl.progress(), 0.25);
    /// ctl.cancel();
    /// assert!(ctl.cancelled());
    /// ```
    pub fn new(publish: impl Fn(&Results) + Send + Sync + 'static) -> Self {
        JobControl {
            progress: Arc::new(AtomicU32::new(0f32.to_bits())),
            cancel: Arc::new(AtomicBool::new(false)),
            publish: Arc::new(Box::new(publish)),
        }
    }
    /// Sets the progress, from 0 (not started) to 1 (done). A value outside that range is held to it, and a value that is not
    /// a number is ignored.
    pub fn set_progress(&self, done: f32) {
        if done.is_finite() {
            self.progress
                .store(done.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
        }
    }
    /// The progress, from 0 to 1.
    pub fn progress(&self) -> f32 {
        f32::from_bits(self.progress.load(Ordering::Relaxed))
    }
    /// Asks the job to stop. The job sees it at its next look at [`JobControl::cancelled`].
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
    /// True once the host has requested a stop.
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
    /// Publishes results before the job ends, for a host that shows the job as it works.
    pub fn publish(&self, results: &Results) {
        (self.publish)(results)
    }
}
