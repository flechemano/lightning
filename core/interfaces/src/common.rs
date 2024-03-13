use std::future::Future;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub use fdi::{Cloned, Consume, Ref, RefMut};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::Notify;
use tracing::trace;

struct ShutdownInner {
    notify: Notify,
    is_shutdown: AtomicBool,
    tx: UnboundedSender<()>,
}

/// Controller utility for shutdown
pub struct ShutdownController {
    inner: Arc<ShutdownInner>,
    rx: UnboundedReceiver<()>,
}

impl Default for ShutdownController {
    fn default() -> Self {
        let (tx, rx) = unbounded_channel();
        Self {
            inner: ShutdownInner {
                notify: Notify::default(),
                is_shutdown: false.into(),
                tx,
            }
            .into(),
            rx,
        }
    }
}

impl ShutdownController {
    /// Create a new shutdown inner.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a new waiter utility
    pub fn waiter(&self) -> ShutdownWaiter {
        ShutdownWaiter(self.inner.clone())
    }

    /// Trigger the shutdown signal and wait for all the child [`ShutdownWaiter`]'s are dropped.
    /// This function WILL hang if any waiters are not correctly dropped on shutdown.
    pub async fn shutdown(&mut self) {
        // Set the shutdown boolean to true, preventing any calls to shutdown and causing
        // all waiters to immediately return in the future
        if self
            .inner
            .is_shutdown
            .swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            panic!("cannot call shutdown more than once")
        }

        // Release all pending shutdown waiters
        self.inner.notify.notify_waiters();

        // Wait for all extra strong references to the shared inner to drop.
        // There are two accounted for references, one in the controller,
        // and one waiter that the provider uses as a dependency..
        let mut count;
        while {
            count = Arc::strong_count(&self.inner);
            count > 2
        } {
            trace!("Waiting for {} shutdown waiter(s) to drop", count - 2);
            self.rx
                .recv()
                .await
                .expect("failed to wait for next waiter drop signal");
        }
    }
}

/// Waiter utility for shutdown
#[derive(Clone)]
pub struct ShutdownWaiter(Arc<ShutdownInner>);

impl ShutdownWaiter {
    /// Standalone function to wait until the shutdown signal is received.
    /// This function is 100% cancel safe and will always return immediately
    /// if shutdown has already happened.
    pub async fn wait_for_shutdown(&self) {
        if self
            .0
            .is_shutdown
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            // There was a missed notify event, so we immediately return
            return;
        }

        self.0.notify.notified().await
    }

    /// Run a function until a shutdown signal is received.
    ///
    /// This method is recommended to use for run loops that are spawned, since the notify permit
    /// will persist then entire time until the run loop future resolves, and will be polled any
    /// time the run loop yields back to the async executor allowing very fast immediate exits.
    ///
    /// This should be considered on a case by case basis, as sometimes it's desirable to fully
    /// handle a branch before checking and exiting on shutdown. For example, maybe a piece of
    /// code than handles a write ahead log on disk might need to be ensured to always complete
    /// if it's doing work, so that no items are lost during a shutdown.
    pub async fn run_until_shutdown<T>(&self, fut: impl Future<Output = T>) -> Option<T> {
        tokio::select! {
            biased;
            _ = self.wait_for_shutdown() => None,
            res = fut => Some(res)
        }
    }
}

impl Drop for ShutdownWaiter {
    fn drop(&mut self) {
        // Send drop signal only if shutdown has been triggered
        if self
            .0
            .is_shutdown
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            self.0.tx.send(()).ok();
        }
    }
}

pub trait WithStartAndShutdown {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool;

    /// Start the system, should not do anything if the system is already
    /// started.
    fn start(&self) -> impl Future<Output = ()> + Send;

    /// Send the shutdown signal to the system.
    fn shutdown(&self) -> impl Future<Output = ()> + Send;
}

/// Any object that implements the cryptographic digest function, this should
/// use a collision resistant hash function and have a representation agnostic
/// hashing for our core objects. Re-exported from [`ink_quill`]
pub use ink_quill::ToDigest;
pub use ink_quill::TranscriptBuilder;

impl<T> WithStartAndShutdown for infusion::Blank<T> {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        true
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {}

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {}
}
