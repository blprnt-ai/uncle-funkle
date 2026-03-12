use crate::model::{Confidence, Issue, IssueSource, IssueStatus, ScoreSnapshot, State, Stats, Tier};
use crate::util::{clamp_score, round1};

pub fn recompute_scores(state: &mut State) -> ScoreSnapshot {
    let mut stats = Stats::default();

    if let Some(metadata) = &state.scan_metadata {
        stats.candidate_files = metadata.candidate_files;
        stats.scanned_files = metadata.scanned_files;
        stats.skipped_files = metadata.skipped_files;
        stats.total_lines = metadata.total_lines;
        stats.total_bytes = metadata.total_bytes;
    }

    let mut objective_penalty = 0.0f32;
    let mut strict_penalty = 0.0f32;
    let mut t3_open = 0usize;
    let mut t4_open = 0usize;
    let mut reopened_open = 0usize;

    for issue in state.issues.values() {
        stats.total_issues += 1;
        *stats
            .issues_by_detector
            .entry(issue.detector.clone())
            .or_insert(0) += 1;
        *stats
            .issues_by_tier
            .entry(issue.tier.as_str().to_string())
            .or_insert(0) += 1;
        *stats
            .issues_by_status
            .entry(issue.status.as_str().to_string())
            .or_insert(0) += 1;

        match issue.source {
            IssueSource::Mechanical => stats.mechanical_issues += 1,
            IssueSource::Subjective => stats.subjective_issues += 1,
        }

        match issue.status {
            IssueStatus::Open => stats.open_issues += 1,
            IssueStatus::Resolved => stats.resolved_issues += 1,
            IssueStatus::Deferred => stats.deferred_issues += 1,
            IssueStatus::Dismissed => stats.dismissed_issues += 1,
        }

        if issue_is_active(issue) {
            strict_penalty += issue_penalty(issue, true);
            if issue.source == IssueSource::Mechanical {
                objective_penalty += issue_penalty(issue, false);
            }

            if issue.reopen_count > 0 {
                reopened_open += 1;
            }
            if issue.tier == Tier::T3 {
                t3_open += 1;
            }
            if issue.tier == Tier::T4 {
                t4_open += 1;
            }
        }
    }

    let scope = (stats.scanned_files.max(1) as f32).sqrt().max(1.0);
    let objective = clamp_score(100.0 - ((objective_penalty * 2.8) / scope));
    let mut strict = clamp_score(100.0 - ((strict_penalty * 3.4) / scope));
    strict = clamp_score(strict - (t3_open as f32 * 1.5) - (t4_open as f32 * 4.0));
    strict = clamp_score(strict - (reopened_open as f32 * 0.75));

    let subjective_average = subjective_average(state);
    let overall = if let Some(subjective) = subjective_average {
        clamp_score(objective * 0.65 + subjective * 0.35)
    } else {
        objective
    };
    let verified = if let Some(subjective) = subjective_average {
        strict.min(subjective)
    } else {
        strict
    };

    let snapshot = ScoreSnapshot {
        overall: round1(overall),
        objective: round1(objective),
        strict: round1(strict),
        verified: round1(verified),
    };

    state.stats = stats;
    state.overall_score = snapshot.overall;
    state.objective_score = snapshot.objective;
    state.strict_score = snapshot.strict;
    state.verified_strict_score = snapshot.verified;

    snapshot
}

fn issue_is_active(issue: &Issue) -> bool {
    matches!(issue.status, IssueStatus::Open | IssueStatus::Deferred)
}

fn issue_penalty(issue: &Issue, strict: bool) -> f32 {
    let tier_weight = match issue.tier {
        Tier::T1 => 0.75,
        Tier::T2 => 1.5,
        Tier::T3 => 3.0,
        Tier::T4 => 5.0,
    };

    let source_weight = match issue.source {
        IssueSource::Mechanical => 1.0,
        IssueSource::Subjective => 1.15,
    };

    let status_weight = match issue.status {
        IssueStatus::Open => 1.0,
        IssueStatus::Deferred => 0.65,
        IssueStatus::Resolved | IssueStatus::Dismissed => 0.0,
    };

    let reopen_weight = if strict {
        1.0 + issue.reopen_count as f32 * 0.15
    } else {
        1.0
    };

    tier_weight * issue.confidence.multiplier() * source_weight * status_weight * reopen_weight
}

fn subjective_average(state: &State) -> Option<f32> {
    if state.subjective_assessments.is_empty() {
        return None;
    }

    let total = state
        .subjective_assessments
        .values()
        .map(|assessment| assessment.score)
        .sum::<f32>();
    let average = total / state.subjective_assessments.len() as f32;
    Some(clamp_score(average))
}

#[allow(dead_code)]
fn _confidence_multiplier(confidence: Confidence) -> f32 {
    confidence.multiplier()
}
