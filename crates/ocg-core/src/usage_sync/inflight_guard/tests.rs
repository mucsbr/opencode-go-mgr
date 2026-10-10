use super::*;

#[tokio::test]
async fn cancelling_the_last_waiter_releases_the_global_slot_for_another_account() {
    let state = FakeUsageHost::new();
    state.insert_ready_go("cancelled", "synthetic-a");
    state.insert_ready_go("other", "synthetic-b");
    let entered = Arc::new(Notify::new());
    let entered_fetch = entered.clone();
    state.inner.runtime.set_fetch_for_test(move |_, key| {
        let entered = entered_fetch.clone();
        Box::pin(async move {
            if key == "synthetic-a" {
                entered.notify_one();
                std::future::pending::<()>().await;
            }
            Ok(sample_snapshot())
        })
    });
    let worker = state.clone();
    let first = tokio::spawn(async move {
        refresh_official_usage(&worker, "cancelled", UsageSyncTrigger::Manual).await
    });
    tokio::time::timeout(StdDuration::from_secs(1), entered.notified())
        .await
        .unwrap();
    first.abort();
    assert!(first.await.unwrap_err().is_cancelled());
    assert!(state.inner.runtime.inflight.lock().is_empty());
    assert!(state.inner.runtime.global.try_lock().is_ok());
    assert!(
        state.inner.sync.lock()["cancelled"]
            .last_attempt_at
            .is_none()
    );
    tokio::time::timeout(
        StdDuration::from_secs(1),
        refresh_official_usage(&state, "other", UsageSyncTrigger::Manual),
    )
    .await
    .unwrap()
    .unwrap();
}

#[tokio::test]
async fn cancelling_one_waiter_keeps_the_surviving_waiter_and_its_leader_authorization() {
    let state = FakeUsageHost::new();
    state.insert_ready_go("shared", "synthetic-shared");
    let authorization = state.guarded_authorization();
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let calls = Arc::new(AtomicUsize::new(0));
    let (entered_fetch, release_fetch, calls_fetch) =
        (entered.clone(), release.clone(), calls.clone());
    state.inner.runtime.set_fetch_for_test(move |_, _| {
        calls_fetch.fetch_add(1, AtomicOrdering::SeqCst);
        let (entered, release) = (entered_fetch.clone(), release_fetch.clone());
        Box::pin(async move {
            entered.notify_one();
            release.notified().await;
            Ok(sample_snapshot())
        })
    });
    let worker = state.clone();
    let leader = tokio::spawn(async move {
        refresh_official_usage_with_authorization(
            &worker,
            "shared",
            UsageSyncTrigger::Manual,
            authorization,
        )
        .await
    });
    tokio::time::timeout(StdDuration::from_secs(1), entered.notified())
        .await
        .unwrap();
    let worker = state.clone();
    let follower = tokio::spawn(async move {
        refresh_official_usage_with_authorization(
            &worker,
            "shared",
            UsageSyncTrigger::Manual,
            UsageSyncCommitAuthorization::Unconditional,
        )
        .await
    });
    tokio::time::timeout(StdDuration::from_secs(1), async {
        loop {
            if state
                .inner
                .runtime
                .inflight
                .lock()
                .get("shared")
                .is_some_and(|entry| entry.waiters == 2)
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    leader.abort();
    assert!(leader.await.unwrap_err().is_cancelled());
    assert_eq!(state.inner.runtime.inflight.lock()["shared"].waiters, 1);
    assert!(state.inner.runtime.global.try_lock().is_err());
    release.notify_one();
    let observation = tokio::time::timeout(StdDuration::from_secs(1), follower)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(observation.owner_authorization, authorization);
    observation.result.unwrap();
    assert_eq!(calls.load(AtomicOrdering::SeqCst), 1);
    assert!(state.inner.runtime.inflight.lock().is_empty());
}

#[test]
fn a_cancelled_old_waiter_cannot_remove_a_new_generation() {
    let runtime = UsageSyncRuntime::new();
    let waiter = super::super::inflight_guard::InflightWaiter::new(&runtime, "same", 1);
    runtime.inflight.lock().insert(
        "same".into(),
        InflightEntry {
            generation: 2,
            waiters: 1,
            future: std::future::pending::<Arc<RefreshResult>>()
                .boxed()
                .shared(),
            authorization: UsageSyncCommitAuthorization::Unconditional,
        },
    );
    drop(waiter);
    assert_eq!(runtime.inflight.lock()["same"].generation, 2);
    assert_eq!(runtime.inflight.lock()["same"].waiters, 1);
}
