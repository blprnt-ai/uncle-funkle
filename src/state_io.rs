use std::path::Path;

use tokio::fs;

use crate::config::Config;
use crate::error::{Result, UncleFunkleError};
use crate::model::State;
use crate::scoring::recompute_scores;
use crate::util::now_rfc3339;

pub async fn load_state_from_root(root: &Path, config: &Config) -> Result<State> {
    let path = config.state_file(root);
    load_state_from_file(&path).await
}

pub async fn save_state_to_root(root: &Path, config: &Config, state: &State) -> Result<()> {
    let path = config.state_file(root);
    save_state_to_file(&path, state).await
}

pub async fn load_state_from_file(path: &Path) -> Result<State> {
    let bytes = match fs::read(path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(State::new()),
        Err(error) => return Err(UncleFunkleError::io(path, error)),
    };

    let mut state: State = serde_json::from_slice(&bytes)?;
    repair_state(&mut state);
    Ok(state)
}

pub async fn save_state_to_file(path: &Path, state: &State) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        UncleFunkleError::InvalidState(format!("state path has no parent: {}", path.display()))
    })?;

    fs::create_dir_all(parent)
        .await
        .map_err(|error| UncleFunkleError::io(parent, error))?;

    let payload = serde_json::to_vec_pretty(state)?;
    fs::write(path, payload)
        .await
        .map_err(|error| UncleFunkleError::io(path, error))
}

pub fn repair_state(state: &mut State) {
    if state.version == 0 {
        state.version = 1;
    }
    if state.created.is_empty() {
        state.created = now_rfc3339();
    }

    for (issue_id, issue) in state.issues.iter_mut() {
        if issue.id.is_empty() {
            issue.id = issue_id.clone();
        }
        if issue.first_seen.is_empty() {
            issue.first_seen = state.created.clone();
        }
        if issue.last_seen.is_empty() {
            issue.last_seen = issue.first_seen.clone();
        }
    }

    recompute_scores(state);
}
