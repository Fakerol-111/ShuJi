use std::collections::HashMap;
use std::sync::Mutex;

use chrono::Utc;
use serde::Serialize;

/// Live state for the current workflow round.
#[derive(Debug, Clone, Serialize)]
pub struct RoundMetricState {
    /// Unix timestamp (millis) when the round started.
    pub started_at: i64,
    /// The currently active department (Chinese name).
    pub current_role: String,
    /// The current skill 内阁 is using (e.g. "workflow_standard").
    pub skill: String,
    /// Cumulative prompt tokens consumed this round.
    pub prompt_tokens: u64,
    /// Cumulative completion tokens consumed this round.
    pub completion_tokens: u64,
    /// Cumulative total tokens.
    pub total_tokens: u64,
    /// Iteration count per department (Chinese name → count).
    pub dept_iterations: HashMap<String, u32>,
}

static ROUND: Mutex<Option<RoundMetricState>> = Mutex::new(None);

/// Start a new round, discarding any previous state.
pub fn start_round() {
    if let Ok(mut state) = ROUND.lock() {
        *state = Some(RoundMetricState {
            started_at: Utc::now().timestamp_millis(),
            current_role: String::new(),
            skill: String::new(),
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            dept_iterations: HashMap::new(),
        });
    }
}

/// Add token usage to the current round.
pub fn add_tokens(prompt: u64, completion: u64) {
    if let Ok(mut state) = ROUND.lock() {
        if let Some(ref mut s) = *state {
            s.prompt_tokens += prompt;
            s.completion_tokens += completion;
            s.total_tokens += prompt + completion;
        }
    }
}

/// Set the currently active department.
pub fn set_role(role: &str) {
    if let Ok(mut state) = ROUND.lock() {
        if let Some(ref mut s) = *state {
            s.current_role = role.to_string();
        }
    }
}

/// Set the current skill name.
pub fn set_skill(skill: &str) {
    if let Ok(mut state) = ROUND.lock() {
        if let Some(ref mut s) = *state {
            s.skill = skill.to_string();
        }
    }
}

/// Increment iteration count for a department.
pub fn tick_iteration(role: &str) {
    if let Ok(mut state) = ROUND.lock() {
        if let Some(ref mut s) = *state {
            *s.dept_iterations.entry(role.to_string()).or_insert(0) += 1;
        }
    }
}

/// Snapshot the current round state.
pub fn snapshot() -> Option<RoundMetricState> {
    ROUND.lock().ok().and_then(|s| s.clone())
}

/// Get the name of the currently active role, if any.
pub fn current_role_name() -> Option<String> {
    ROUND.lock().ok().and_then(|s| {
        s.as_ref().map(|r| r.current_role.clone()).filter(|r| !r.is_empty())
    })
}
