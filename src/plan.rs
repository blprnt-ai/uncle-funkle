use std::collections::BTreeMap;

use crate::model::{Issue, IssueStatus, Plan, PlanItem, State, Tier};
use crate::util::{humanize_detector, now_rfc3339};

#[derive(Debug, Default)]
struct GroupAccumulator {
    detector: String,
    file: Option<String>,
    tier: Tier,
    priority: u32,
    summaries: Vec<String>,
    issue_ids: Vec<String>,
}

pub fn build_plan(state: &State) -> Plan {
    let mut grouped: BTreeMap<String, GroupAccumulator> = BTreeMap::new();

    for issue in state.issues.values() {
        if !matches!(issue.status, IssueStatus::Open | IssueStatus::Deferred) {
            continue;
        }

        let key = grouping_key(issue);
        let entry = grouped.entry(key).or_default();
        entry.detector = issue.detector.clone();
        entry.file = if issue.path.is_empty() {
            None
        } else {
            Some(issue.path.clone())
        };
        if issue.tier > entry.tier {
            entry.tier = issue.tier;
        }
        entry.priority += priority_for_issue(issue);
        entry.issue_ids.push(issue.id.clone());
        if !issue.summary.is_empty() {
            entry.summaries.push(issue.summary.clone());
        }
    }

    let mut items: Vec<PlanItem> = grouped
        .into_iter()
        .map(|(key, group)| {
            let title = match &group.file {
                Some(file) => format!("{} in {}", humanize_detector(&group.detector), file),
                None => humanize_detector(&group.detector),
            };

            let summary = if group.summaries.is_empty() {
                title.clone()
            } else {
                group.summaries[0].clone()
            };

            let resolve_hint = format!(
                "resolve these issue ids after fixing: {}",
                group.issue_ids.join(", ")
            );

            PlanItem {
                key,
                title,
                summary,
                detector: group.detector.clone(),
                file: group.file.clone(),
                tier: group.tier,
                priority: group.priority,
                issue_count: group.issue_ids.len(),
                issue_ids: group.issue_ids,
                guidance: guidance_for_detector(&group.detector).to_string(),
                resolve_hint,
            }
        })
        .collect();

    items.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| right.tier.cmp(&left.tier))
            .then_with(|| left.title.cmp(&right.title))
    });

    Plan {
        generated_at: now_rfc3339(),
        items,
    }
}

pub fn next_item(state: &State) -> Option<PlanItem> {
    build_plan(state).items.into_iter().next()
}

fn grouping_key(issue: &Issue) -> String {
    if issue.detector == "duplicate_block" {
        return issue.id.clone();
    }

    if issue.path.is_empty() {
        return issue.detector.clone();
    }

    format!("{}::{}", issue.path, issue.detector)
}

fn priority_for_issue(issue: &Issue) -> u32 {
    let mut total = issue.tier.weight() * 100 + issue.confidence.priority_points();

    if matches!(issue.status, IssueStatus::Deferred) {
        total = total.saturating_sub(20);
    }

    total + issue.reopen_count * 3
}

fn guidance_for_detector(detector: &str) -> &'static str {
    match detector {
        "todo_comment" => {
            "Replace placeholder comments with concrete work or remove them once the implementation is complete."
        }
        "large_file" => {
            "Split the file by responsibility. Extract cohesive modules or types and keep top-level orchestration thin."
        }
        "long_function" => {
            "Break the function into named helpers. Separate decision logic from side effects and use smaller units."
        }
        "deep_nesting" => {
            "Flatten control flow with guard clauses, early returns, or helper functions to keep nesting shallow."
        }
        "branch_density" => {
            "Reduce branching by extracting policy decisions, introducing dispatch tables, or separating phases."
        }
        "duplicate_block" => {
            "Extract the repeated block into a shared abstraction so each call site becomes a thin wrapper."
        }
        "long_line" => {
            "Wrap or restructure long expressions so each line communicates one idea and stays reviewable."
        }
        "debug_artifact" => {
            "Remove ad-hoc debug output or route it through a deliberate logging or tracing interface."
        }
        detector if detector.starts_with("subjective.") => {
            "Address the design concern holistically. Prefer renaming, boundary cleanup, and simpler abstractions over cosmetic edits."
        }
        _ => {
            "Fix the issue in a durable way, then rescan so the queue can reprioritize the remaining work."
        }
    }
}
