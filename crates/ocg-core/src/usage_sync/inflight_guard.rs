//! Last-waiter cancellation must release the shared fetch and its global lock.
use super::UsageSyncRuntime;

pub(super) struct InflightWaiter<'a> {
    runtime: &'a UsageSyncRuntime,
    account_id: String,
    generation: u64,
}

impl<'a> InflightWaiter<'a> {
    pub(super) fn new(runtime: &'a UsageSyncRuntime, account_id: &str, generation: u64) -> Self {
        Self {
            runtime,
            account_id: account_id.into(),
            generation,
        }
    }
}

impl Drop for InflightWaiter<'_> {
    fn drop(&mut self) {
        let removed = {
            let mut inflight = self.runtime.inflight.lock();
            let Some(entry) = inflight.get_mut(&self.account_id) else {
                return;
            };
            if entry.generation != self.generation {
                return;
            }
            entry.waiters -= 1;
            if entry.waiters == 0 {
                inflight.remove(&self.account_id)
            } else {
                None
            }
        };
        // Drop captured host state and the fetch outside the registry lock.
        drop(removed);
    }
}
