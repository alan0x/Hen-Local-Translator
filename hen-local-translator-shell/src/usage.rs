use chrono::{Datelike, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

const DEFAULT_COMPARISON_RATE_PER_MINUTE: f64 = 1.50;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct StoredUsage {
    lifetime_seconds: u64,
    month_key: String,
    monthly_seconds: u64,
    completed_sessions: u64,
    comparison_rate_per_minute: f64,
    session_active: bool,
}

impl Default for StoredUsage {
    fn default() -> Self {
        Self {
            lifetime_seconds: 0,
            month_key: current_month_key(),
            monthly_seconds: 0,
            completed_sessions: 0,
            comparison_rate_per_minute: DEFAULT_COMPARISON_RATE_PER_MINUTE,
            session_active: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    pub current_session_seconds: u64,
    pub monthly_seconds: u64,
    pub lifetime_seconds: u64,
    pub completed_sessions: u64,
    pub comparison_rate_per_minute: f64,
    pub estimated_value: f64,
    pub running: bool,
    pub month_key: String,
}

struct UsageState {
    stored: StoredUsage,
    session_started: Option<Instant>,
    persisted_session_seconds: u64,
}

pub struct UsageTracker {
    path: PathBuf,
    state: Mutex<UsageState>,
}

impl UsageTracker {
    pub fn load(path: PathBuf) -> Arc<Self> {
        let mut stored = fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str::<StoredUsage>(&content).ok())
            .unwrap_or_default();
        stored.session_active = false;
        sanitize(&mut stored);
        Arc::new(Self {
            path,
            state: Mutex::new(UsageState {
                stored,
                session_started: None,
                persisted_session_seconds: 0,
            }),
        })
    }

    pub fn start(&self) -> Result<(), String> {
        let mut state = self.state.lock();
        roll_month(&mut state.stored);
        if state.session_started.is_none() {
            state.session_started = Some(Instant::now());
            state.persisted_session_seconds = 0;
            state.stored.session_active = true;
            self.persist_locked(&state)?;
        }
        Ok(())
    }

    pub fn checkpoint(&self) -> Result<(), String> {
        let mut state = self.state.lock();
        roll_month(&mut state.stored);
        let elapsed = elapsed_seconds(&state);
        let delta = elapsed.saturating_sub(state.persisted_session_seconds);
        if delta > 0 {
            state.stored.lifetime_seconds += delta;
            state.stored.monthly_seconds += delta;
            state.persisted_session_seconds = elapsed;
            self.persist_locked(&state)?;
        }
        Ok(())
    }

    pub fn stop(&self) -> Result<(), String> {
        let mut state = self.state.lock();
        roll_month(&mut state.stored);
        let elapsed = elapsed_seconds(&state);
        let delta = elapsed.saturating_sub(state.persisted_session_seconds);
        if state.session_started.is_some() {
            state.stored.lifetime_seconds += delta;
            state.stored.monthly_seconds += delta;
            state.stored.completed_sessions += 1;
        }
        state.session_started = None;
        state.persisted_session_seconds = 0;
        state.stored.session_active = false;
        self.persist_locked(&state)
    }

    pub fn snapshot(&self) -> UsageSnapshot {
        let mut state = self.state.lock();
        roll_month(&mut state.stored);
        let current = elapsed_seconds(&state);
        let unpersisted = current.saturating_sub(state.persisted_session_seconds);
        let lifetime = state.stored.lifetime_seconds + unpersisted;
        UsageSnapshot {
            current_session_seconds: current,
            monthly_seconds: state.stored.monthly_seconds + unpersisted,
            lifetime_seconds: lifetime,
            completed_sessions: state.stored.completed_sessions,
            comparison_rate_per_minute: state.stored.comparison_rate_per_minute,
            estimated_value: (lifetime as f64 / 60.0) * state.stored.comparison_rate_per_minute,
            running: state.session_started.is_some(),
            month_key: state.stored.month_key.clone(),
        }
    }

    pub fn set_comparison_rate(&self, rate: f64) -> Result<UsageSnapshot, String> {
        if !rate.is_finite() || !(0.0..=100.0).contains(&rate) {
            return Err("Comparison rate must be between $0 and $100 per minute".into());
        }
        let mut state = self.state.lock();
        state.stored.comparison_rate_per_minute = rate;
        self.persist_locked(&state)?;
        drop(state);
        Ok(self.snapshot())
    }

    fn persist_locked(&self, state: &UsageState) -> Result<(), String> {
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create usage directory: {error}"))?;
        let json = serde_json::to_vec_pretty(&state.stored)
            .map_err(|error| format!("Could not serialize usage: {error}"))?;
        let temporary = self.path.with_extension("json.tmp");
        fs::write(&temporary, json).map_err(|error| format!("Could not save usage: {error}"))?;
        fs::rename(&temporary, &self.path)
            .map_err(|error| format!("Could not finish saving usage: {error}"))
    }
}

pub fn start_checkpoint_loop(tracker: Arc<UsageTracker>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(30));
        if let Err(error) = tracker.checkpoint() {
            log::error!("Could not checkpoint local usage: {error}");
        }
    });
}

fn elapsed_seconds(state: &UsageState) -> u64 {
    state
        .session_started
        .map(|started| started.elapsed().as_secs())
        .unwrap_or(0)
}

fn current_month_key() -> String {
    let now = Utc::now();
    format!("{:04}-{:02}", now.year(), now.month())
}

fn roll_month(stored: &mut StoredUsage) {
    let current = current_month_key();
    if stored.month_key != current {
        stored.month_key = current;
        stored.monthly_seconds = 0;
    }
}

fn sanitize(stored: &mut StoredUsage) {
    if !stored.comparison_rate_per_minute.is_finite()
        || !(0.0..=100.0).contains(&stored.comparison_rate_per_minute)
    {
        stored.comparison_rate_per_minute = DEFAULT_COMPARISON_RATE_PER_MINUTE;
    }
    if stored.month_key.is_empty() {
        stored.month_key = current_month_key();
    }
    roll_month(stored);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_comparison_rate_is_sanitized() {
        let mut stored = StoredUsage {
            comparison_rate_per_minute: f64::NAN,
            ..StoredUsage::default()
        };
        sanitize(&mut stored);
        assert_eq!(stored.comparison_rate_per_minute, 1.5);
    }

    #[test]
    fn snapshot_value_uses_visible_rate() {
        let directory =
            std::env::temp_dir().join(format!("hen-local-usage-test-{}", std::process::id()));
        let tracker = UsageTracker::load(directory.join("usage.json"));
        {
            let mut state = tracker.state.lock();
            state.stored.lifetime_seconds = 120;
            state.stored.comparison_rate_per_minute = 2.0;
        }
        assert_eq!(tracker.snapshot().estimated_value, 4.0);
        let _ = fs::remove_dir_all(directory);
    }
}
