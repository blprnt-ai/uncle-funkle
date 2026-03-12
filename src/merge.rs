use std::collections::BTreeSet;

use crate::model::{
    AssessmentImport, Finding, Issue, IssueSource, IssueStatus, Location, ScoreSnapshot,
    SubjectiveAssessment, SubjectiveFindingImport, ScanMetadata, ScanReport, State,
};
use crate::scoring::recompute_scores;
use crate::util::{now_rfc3339, stable_hash, stable_issue_id};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(default)]
pub struct MergeSummary {
    pub added: usize,
    pub updated: usize,
    pub reopened: usize,
    pub auto_resolved: usize,
    pub total_open: usize,
    pub scores: ScoreSnapshot,
    pub metadata: Option<ScanMetadata>,
}

impl Default for MergeSummary {
    fn default() -> Self {
        Self {
            added: 0,
            updated: 0,
            reopened: 0,
            auto_resolved: 0,
            total_open: 0,
            scores: ScoreSnapshot::default(),
            metadata: None,
        }
    }
}

pub fn merge_scan_report(state: &mut State, report: ScanReport) -> MergeSummary {
    let seen_at = report.generated_at.clone();
    let active_ids: BTreeSet<String> = report.findings.iter().map(|finding| finding.id.clone()).collect();
    let previous_mechanical_ids: Vec<String> = state
        .issues
        .iter()
        .filter_map(|(id, issue)| {
            (issue.source == IssueSource::Mechanical
                && matches!(issue.status, IssueStatus::Open | IssueStatus::Deferred))
            .then(|| id.clone())
        })
        .collect();

    let mut summary = MergeSummary {
        metadata: Some(report.metadata.clone()),
        ..MergeSummary::default()
    };

    for finding in report.findings {
        match state.issues.get_mut(&finding.id) {
            Some(existing) => {
                let was_resolved = existing.status == IssueStatus::Resolved;
                apply_finding_to_issue(existing, &finding, &seen_at);
                if was_resolved {
                    existing.status = IssueStatus::Open;
                    existing.resolved_at = None;
                    existing.reopen_count = existing.reopen_count.saturating_add(1);
                    summary.reopened += 1;
                }
                summary.updated += 1;
            }
            None => {
                let issue = issue_from_finding(finding, &seen_at);
                state.issues.insert(issue.id.clone(), issue);
                summary.added += 1;
            }
        }
    }

    for issue_id in previous_mechanical_ids {
        if active_ids.contains(&issue_id) {
            continue;
        }

        if let Some(issue) = state.issues.get_mut(&issue_id) {
            issue.status = IssueStatus::Resolved;
            issue.resolved_at = Some(seen_at.clone());
            issue.last_seen = seen_at.clone();
            summary.auto_resolved += 1;
        }
    }

    state.scan_metadata = Some(report.metadata.clone());
    state.last_scan = Some(seen_at);
    state.scan_count = state.scan_count.saturating_add(1);
    summary.scores = recompute_scores(state);
    summary.total_open = state.stats.open_issues;
    summary
}

pub fn import_assessment(state: &mut State, import: AssessmentImport) -> MergeSummary {
    let imported_at = now_rfc3339();
    let detector = format!("subjective.{}", import.dimension);
    let active_ids: BTreeSet<String> = import
        .findings
        .iter()
        .enumerate()
        .map(|(index, finding)| subjective_finding_to_id(&import.dimension, index, finding))
        .collect();

    let previous_subjective_ids: Vec<String> = state
        .issues
        .iter()
        .filter_map(|(id, issue)| {
            (issue.detector == detector && matches!(issue.status, IssueStatus::Open | IssueStatus::Deferred))
                .then(|| id.clone())
        })
        .collect();

    let mut summary = MergeSummary::default();
    let mut finding_ids = Vec::with_capacity(import.findings.len());

    for (index, finding) in import.findings.iter().enumerate() {
        let materialized = finding_from_subjective(&import.dimension, index, finding);
        finding_ids.push(materialized.id.clone());
        match state.issues.get_mut(&materialized.id) {
            Some(existing) => {
                let was_resolved = existing.status == IssueStatus::Resolved;
                apply_finding_to_issue(existing, &materialized, &imported_at);
                if was_resolved {
                    existing.status = IssueStatus::Open;
                    existing.resolved_at = None;
                    existing.reopen_count = existing.reopen_count.saturating_add(1);
                    summary.reopened += 1;
                }
                summary.updated += 1;
            }
            None => {
                let issue = issue_from_finding(materialized, &imported_at);
                state.issues.insert(issue.id.clone(), issue);
                summary.added += 1;
            }
        }
    }

    for issue_id in previous_subjective_ids {
        if active_ids.contains(&issue_id) {
            continue;
        }

        if let Some(issue) = state.issues.get_mut(&issue_id) {
            issue.status = IssueStatus::Resolved;
            issue.resolved_at = Some(imported_at.clone());
            issue.last_seen = imported_at.clone();
            summary.auto_resolved += 1;
        }
    }

    state.subjective_assessments.insert(
        import.dimension.clone(),
        SubjectiveAssessment {
            dimension: import.dimension,
            score: import.score,
            summary: import.summary,
            imported_at,
            metadata: import.metadata,
            finding_ids,
        },
    );

    summary.scores = recompute_scores(state);
    summary.total_open = state.stats.open_issues;
    summary
}

pub fn resolve_issue(state: &mut State, issue_id: &str, note: Option<String>) -> bool {
    let now = now_rfc3339();
    if let Some(issue) = state.issues.get_mut(issue_id) {
        issue.status = IssueStatus::Resolved;
        issue.resolved_at = Some(now.clone());
        issue.last_seen = now;
        issue.note = note;
        recompute_scores(state);
        return true;
    }
    false
}

pub fn defer_issue(state: &mut State, issue_id: &str, note: Option<String>) -> bool {
    if let Some(issue) = state.issues.get_mut(issue_id) {
        issue.status = IssueStatus::Deferred;
        issue.note = note;
        recompute_scores(state);
        return true;
    }
    false
}

pub fn dismiss_issue(state: &mut State, issue_id: &str, note: Option<String>) -> bool {
    if let Some(issue) = state.issues.get_mut(issue_id) {
        issue.status = IssueStatus::Dismissed;
        issue.note = note;
        recompute_scores(state);
        return true;
    }
    false
}

pub fn reopen_issue(state: &mut State, issue_id: &str, note: Option<String>) -> bool {
    let now = now_rfc3339();
    if let Some(issue) = state.issues.get_mut(issue_id) {
        issue.status = IssueStatus::Open;
        issue.note = note;
        issue.resolved_at = None;
        issue.last_seen = now;
        issue.reopen_count = issue.reopen_count.saturating_add(1);
        recompute_scores(state);
        return true;
    }
    false
}

fn issue_from_finding(finding: Finding, seen_at: &str) -> Issue {
    Issue {
        id: finding.id,
        fingerprint: finding.fingerprint,
        detector: finding.detector,
        source: finding.source,
        language: finding.language,
        tier: finding.tier,
        confidence: finding.confidence,
        path: finding.path,
        summary: finding.summary,
        description: finding.description,
        location: finding.location,
        detail: finding.detail,
        status: IssueStatus::Open,
        note: None,
        first_seen: seen_at.to_string(),
        last_seen: seen_at.to_string(),
        resolved_at: None,
        reopen_count: 0,
    }
}

fn apply_finding_to_issue(issue: &mut Issue, finding: &Finding, seen_at: &str) {
    issue.fingerprint = finding.fingerprint.clone();
    issue.detector = finding.detector.clone();
    issue.source = finding.source;
    issue.language = finding.language.clone();
    issue.tier = finding.tier;
    issue.confidence = finding.confidence;
    issue.path = finding.path.clone();
    issue.summary = finding.summary.clone();
    issue.description = finding.description.clone();
    issue.location = finding.location.clone();
    issue.detail = finding.detail.clone();
    issue.last_seen = seen_at.to_string();
}

fn finding_from_subjective(dimension: &str, index: usize, finding: &SubjectiveFindingImport) -> Finding {
    let fingerprint = stable_hash([
        "subjective",
        dimension,
        &index.to_string(),
        finding.path.as_deref().unwrap_or(""),
        &finding.summary,
        &finding.description,
        &finding.line_start.unwrap_or_default().to_string(),
        &finding.line_end.unwrap_or_default().to_string(),
    ]);

    Finding {
        id: stable_issue_id(&fingerprint),
        fingerprint,
        detector: format!("subjective.{}", dimension),
        source: IssueSource::Subjective,
        language: crate::model::LanguageKind::Other("review".to_string()),
        tier: finding.tier,
        confidence: finding.confidence,
        path: finding.path.clone().unwrap_or_default(),
        summary: finding.summary.clone(),
        description: finding.description.clone(),
        location: Location {
            line_start: finding.line_start,
            line_end: finding.line_end,
        },
        detail: finding.metadata.clone(),
    }
}

fn subjective_finding_to_id(dimension: &str, index: usize, finding: &SubjectiveFindingImport) -> String {
    let fingerprint = stable_hash([
        "subjective",
        dimension,
        &index.to_string(),
        finding.path.as_deref().unwrap_or(""),
        &finding.summary,
        &finding.description,
        &finding.line_start.unwrap_or_default().to_string(),
        &finding.line_end.unwrap_or_default().to_string(),
    ]);
    stable_issue_id(&fingerprint)
}
