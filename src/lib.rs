mod config;
mod error;
mod merge;
mod model;
mod plan;
mod scan;
mod scoring;
mod state_io;
mod util;

use std::path::Path;

pub use config::{Config, DetectorThresholds};
pub use error::{Result, UncleFunkleError};
pub use merge::{
    defer_issue, dismiss_issue, import_assessment, merge_scan_report, reopen_issue, resolve_issue,
    MergeSummary,
};
pub use model::{
    AssessmentImport, Confidence, FileSummary, Finding, Issue, IssueSource, IssueStatus,
    LanguageKind, Location, Plan, PlanItem, ScanMetadata, ScanReport, ScoreSnapshot, State, Stats,
    SubjectiveAssessment, SubjectiveFindingImport, Tier,
};
pub use plan::{build_plan, next_item};
pub use scan::scan_project;
pub use scoring::recompute_scores;
pub use state_io::{
    load_state_from_file, load_state_from_root, repair_state, save_state_to_file,
    save_state_to_root,
};

#[derive(Debug, Clone)]
pub struct UncleFunkle {
    config: Config,
}

impl UncleFunkle {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub async fn load_state(&self, root: impl AsRef<Path>) -> Result<State> {
        load_state_from_root(root.as_ref(), &self.config).await
    }

    pub async fn save_state(&self, root: impl AsRef<Path>, state: &State) -> Result<()> {
        save_state_to_root(root.as_ref(), &self.config, state).await
    }

    pub async fn scan(&self, root: impl AsRef<Path>) -> Result<ScanReport> {
        scan_project(root.as_ref(), &self.config).await
    }

    pub fn merge_scan(&self, state: &mut State, report: ScanReport) -> MergeSummary {
        merge_scan_report(state, report)
    }

    pub fn import_subjective_assessment(
        &self,
        state: &mut State,
        assessment: AssessmentImport,
    ) -> MergeSummary {
        import_assessment(state, assessment)
    }

    pub fn plan(&self, state: &State) -> Plan {
        build_plan(state)
    }

    pub fn next(&self, state: &State) -> Option<PlanItem> {
        next_item(state)
    }

    pub fn resolve_issue(&self, state: &mut State, issue_id: &str, note: Option<String>) -> bool {
        resolve_issue(state, issue_id, note)
    }

    pub fn defer_issue(&self, state: &mut State, issue_id: &str, note: Option<String>) -> bool {
        defer_issue(state, issue_id, note)
    }

    pub fn dismiss_issue(&self, state: &mut State, issue_id: &str, note: Option<String>) -> bool {
        dismiss_issue(state, issue_id, note)
    }

    pub fn reopen_issue(&self, state: &mut State, issue_id: &str, note: Option<String>) -> bool {
        reopen_issue(state, issue_id, note)
    }
}
