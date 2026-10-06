//! Quota refresh belongs to the native process, including when every WebView is hidden.
use crate::models::AppConfig;
use futures::{future::{BoxFuture, Shared}, FutureExt};
use std::{future::Future, sync::Mutex, time::{Duration, Instant}};

type SharedRefresh<T> = Shared<BoxFuture<'static, Result<T, String>>>;

struct Flight<T> { generation: u64, pending: Option<(u64, SharedRefresh<T>)> }
pub(crate) struct SingleFlight<T> { state: Mutex<Flight<T>> }
impl<T> SingleFlight<T> {
    pub const fn new() -> Self { Self { state: Mutex::new(Flight { generation: 0, pending: None }) } }
}
impl<T: Clone + Send + Sync + 'static> SingleFlight<T> {
    /// Manual and automatic requests share an in-flight batch, including failures.
    pub async fn run<F, Fut>(&self, action: F) -> Result<T, String>
    where F: FnOnce() -> Fut, Fut: Future<Output = Result<T, String>> + Send + 'static {
        let (generation, pending) = {
            let mut state = self.state.lock().map_err(|_| "quota_refresh_lock_unavailable")?;
            if let Some((generation, pending)) = &state.pending { (*generation, pending.clone()) }
            else {
                state.generation += 1;
                let pending = action().boxed().shared();
                let generation = state.generation;
                state.pending = Some((generation, pending.clone()));
                (generation, pending)
            }
        };
        let result = pending.await;
        if let Ok(mut state) = self.state.lock() {
            if state.pending.as_ref().is_some_and(|(current, _)| *current == generation) { state.pending = None; }
        }
        result
    }
}

#[derive(Default)]
struct Schedule { period: Option<Duration>, next_attempt: Option<Instant> }
impl Schedule {
    fn due(&mut self, config: &AppConfig, now: Instant) -> bool {
        let period = (config.auto_refresh && config.refresh_interval > 0).then(|| {
            let seconds = config.refresh_interval as u64 * 60;
            // Leave a small request budget before the presentation's freshness cutoff.
            Duration::from_secs(seconds - (seconds / 10).min(30))
        });
        if period != self.period { self.period = period; self.next_attempt = None; }
        let Some(period) = period else { return false; };
        if self.next_attempt.is_some_and(|next| now < next) { return false; }
        // No catch-up burst after sleep, and failed rounds wait for the next period.
        self.next_attempt = Some(now + period);
        true
    }
}

pub fn start(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut schedule = Schedule::default();
        let mut timer = tokio::time::interval(Duration::from_secs(5));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            timer.tick().await;
            let Ok(config) = super::load_app_config() else { continue; };
            if !schedule.due(&config, Instant::now()) { continue; }
            if crate::commands::refresh_all_quotas_internal(Some(app.clone())).await.is_err() {
                super::logger::log_warn("Automatic quota refresh failed; cached observations retained");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

    #[test]
    fn opt_out_interval_changes_and_resume_never_create_catch_up_requests() {
        let now = Instant::now(); let mut schedule = Schedule::default();
        let mut config = AppConfig { auto_refresh: false, refresh_interval: 5, ..Default::default() };
        assert!(!schedule.due(&config, now));
        config.auto_refresh = true; assert!(schedule.due(&config, now));
        assert!(!schedule.due(&config, now + Duration::from_secs(269)));
        assert!(schedule.due(&config, now + Duration::from_secs(270)));
        assert!(schedule.due(&config, now + Duration::from_secs(3600)));
        assert!(!schedule.due(&config, now + Duration::from_secs(3600)));
        config.refresh_interval = 1; assert!(schedule.due(&config, now + Duration::from_secs(3601)));
        config.refresh_interval = 0; assert!(!schedule.due(&config, now + Duration::from_secs(4000)));
        config.refresh_interval = -1; assert!(!schedule.due(&config, now + Duration::from_secs(4000)));
        config.refresh_interval = 5; config.auto_refresh = false;
        assert!(!schedule.due(&config, now + Duration::from_secs(5000)));
        config.auto_refresh = true; assert!(schedule.due(&config, now + Duration::from_secs(5000)));
    }

    #[test]
    fn an_hour_without_webview_ticks_keeps_all_five_accounts_fresh() {
        use super::super::{account_dashboard, menu_bar_projection};
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("accounts")).unwrap();
        let ids: Vec<_> = (0..5).map(|n| format!("synthetic-{n}")).collect();
        std::fs::write(root.path().join("accounts.json"), serde_json::json!({
            "version":"2.0", "current_account_id":ids[0], "accounts":ids.iter().map(|id| serde_json::json!({
                "id":id,"email":format!("{id}@example.invalid"),"created_at":1,"last_used":1
            })).collect::<Vec<_>>()
        }).to_string()).unwrap();
        let config = AppConfig { auto_refresh: true, refresh_interval: 5, ..Default::default() };
        let mut schedule = Schedule::default(); let started = Instant::now(); let timestamp = 1_790_000_000;
        let mut rounds = 0;
        for seconds in (0..=3600).step_by(5) {
            if schedule.due(&config, started + Duration::from_secs(seconds as u64)) {
                rounds += 1;
                for id in &ids { std::fs::write(root.path().join(format!("accounts/{id}.json")), serde_json::json!({
                    "id":id,"email":format!("{id}@example.invalid"),"quota":{"models":[],"last_updated":timestamp + seconds,
                        "quota_groups":[{"display_name":"Gemini","buckets":(["5h","weekly"].iter().map(|window| serde_json::json!({
                            "bucket_id":format!("gemini-{window}"),"window":window,"remaining_fraction":0.8,
                            "remaining_fraction_known":true,"reset_time":"2030-01-01T00:00:00Z"
                        })).collect::<Vec<_>>())}]}
                }).to_string()).unwrap(); }
            }
            let snapshot = account_dashboard::snapshot_in_dir(root.path()).unwrap();
            let windows: Vec<_> = snapshot.accounts.iter().map(|account|
                menu_bar_projection::account_windows(account, timestamp + seconds, 5)).collect();
            assert_eq!(menu_bar_projection::aggregate(&windows, crate::models::config::MenuBarQuotaScope::Gemini, 0, 10), (Some(80.0), 5, 5));
        }
        assert_eq!(rounds, 14);
        // Failed refreshes never make the old observations fresh again.
        let snapshot = account_dashboard::snapshot_in_dir(root.path()).unwrap();
        assert!(snapshot.accounts.iter().all(|account|
            menu_bar_projection::account_windows(account, timestamp + 3900, 5) == [[None; 2]; 2]));
    }

    #[tokio::test]
    async fn manual_refresh_joins_the_automatic_batch_and_errors_can_retry() {
        let gate = SingleFlight::<usize>::new(); let calls = Arc::new(AtomicUsize::new(0));
        let (send, receive) = tokio::sync::oneshot::channel();
        let first_calls = calls.clone();
        let automatic = gate.run(|| async move { first_calls.fetch_add(1, Ordering::SeqCst); receive.await.unwrap(); Ok(5) });
        tokio::pin!(automatic);
        assert!(futures::poll!(&mut automatic).is_pending());
        let manual = gate.run(|| async { panic!("must join existing batch") });
        tokio::pin!(manual);
        assert!(futures::poll!(&mut manual).is_pending());
        send.send(()).unwrap();
        assert_eq!(automatic.await, Ok(5)); assert_eq!(manual.await, Ok(5)); assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(gate.run(|| async { Err("synthetic failure".into()) }).await, Err("synthetic failure".into()));
        assert_eq!(gate.run(|| async { Ok(4) }).await, Ok(4));
    }
}
