//! Keep the recorded account aligned with local client observations even when
//! the main WebView is hidden. This scheduler never switches client credentials.
use crate::models::{Account, AppConfig};
use std::time::{Duration, Instant};

static SYNC: super::quota_refresh::SingleFlight<Option<Account>> = super::quota_refresh::SingleFlight::new();

pub async fn synchronize(app: tauri::AppHandle) -> Result<Option<Account>, String> {
    SYNC.run(|| async move { crate::commands::sync_account_from_db_internal(app).await }).await
}

#[derive(Default)]
struct Schedule { period: Option<Duration>, next_attempt: Option<Instant> }
impl Schedule {
    fn due(&mut self, config: &AppConfig, now: Instant) -> bool {
        let period = (config.auto_sync && config.sync_interval > 0)
            .then(|| Duration::from_secs(config.sync_interval as u64 * 60));
        if period != self.period { self.period = period; self.next_attempt = None; }
        let Some(period) = period else { return false; };
        if self.next_attempt.is_some_and(|next| now < next) { return false; }
        // Schedule attempts, including failures; resuming does not catch up.
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
            if synchronize(app.clone()).await.is_err() {
                super::logger::log_warn("Automatic account synchronization unavailable; recorded account retained");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opt_out_reschedule_and_resume_do_not_create_extra_attempts() {
        let now = Instant::now(); let mut schedule = Schedule::default();
        let mut config = AppConfig { auto_sync: false, sync_interval: 5, ..Default::default() };
        assert!(!schedule.due(&config, now));
        config.auto_sync = true;
        assert!(schedule.due(&config, now));
        assert!(!schedule.due(&config, now + Duration::from_secs(299)));
        assert!(schedule.due(&config, now + Duration::from_secs(300)));
        // A failed attempt still waits its full period.
        assert!(!schedule.due(&config, now + Duration::from_secs(301)));
        assert!(schedule.due(&config, now + Duration::from_secs(3600)));
        assert!(!schedule.due(&config, now + Duration::from_secs(3600)));
        config.sync_interval = 1;
        assert!(schedule.due(&config, now + Duration::from_secs(3601)));
        config.auto_sync = false;
        assert!(!schedule.due(&config, now + Duration::from_secs(3661)));
        config.auto_sync = true;
        assert!(schedule.due(&config, now + Duration::from_secs(3661)));
        for minutes in [0, -1] {
            config.sync_interval = minutes;
            assert!(!schedule.due(&config, now + Duration::from_secs(4000)));
        }
    }
    #[test]
    fn an_hour_without_webview_events_uses_the_selected_native_interval() {
        let config = AppConfig { auto_sync: true, sync_interval: 5, ..Default::default() };
        let now = Instant::now(); let mut schedule = Schedule::default();
        let attempts: Vec<_> = (0..=3600).step_by(5)
            .filter(|seconds| schedule.due(&config, now + Duration::from_secs(*seconds))).collect();
        assert_eq!(attempts, (0..=3600).step_by(300).collect::<Vec<_>>());
    }
}
