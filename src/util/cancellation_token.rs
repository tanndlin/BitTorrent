use std::{
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

/// Shared stop flag; clones all observe the same cancellation
#[derive(Clone, Default, Debug)]
pub struct CancellationToken(Arc<(Mutex<bool>, Condvar)>);

impl CancellationToken {
    pub fn cancel(&self) {
        let (cancelled, condvar) = &*self.0;
        *cancelled.lock().unwrap() = true;
        condvar.notify_all();
    }

    pub fn is_cancelled(&self) -> bool {
        *self.0 .0.lock().unwrap()
    }

    /// Sleeps for up to `timeout`, waking early if cancelled. Returns whether cancelled
    pub fn wait_timeout(&self, timeout: Duration) -> bool {
        let (cancelled, condvar) = &*self.0;
        let guard = cancelled.lock().unwrap();
        let (guard, _) = condvar
            .wait_timeout_while(guard, timeout, |cancelled| !*cancelled)
            .unwrap();
        *guard
    }
}
