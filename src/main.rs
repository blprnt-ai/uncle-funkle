use std::cmp::Reverse;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde::Serialize;
use uncle_funkle::{
    Config, Issue, IssueStatus, MergeSummary, State, UncleFunkle, UncleFunkleError,
};

#[derive(Parser, Debug)]
#[command(name = "uncle-funkle")]
#[command(about = "Scan a codebase or inspect saved state")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Scan {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    Status {
        #[arg(default_value = ".")]
        path: PathBuf,
        issue_id: Option<String>,
    },
    List {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        all: bool,
    },
    Next {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    Resolve {
        issue_id: String,
        path: Option<PathBuf>,
    },
    Defer {
        issue_id: String,
        path: Option<PathBuf>,
    },
    Dismiss {
        issue_id: String,
        path: Option<PathBuf>,
    },
    Reopen {
        issue_id: String,
        path: Option<PathBuf>,
    },
}

#[derive(Serialize)]
struct SummaryJsonOutput {
    open_issues: usize,
    added: usize,
    updated: usize,
    reopened: usize,
    auto_resolved: usize,
    scores: uncle_funkle::ScoreSnapshot,
    next: Option<Issue>,
}

#[derive(Serialize)]
struct ListJsonOutput {
    issues: Vec<Issue>,
}

#[derive(Serialize)]
struct IssueJsonOutput {
    issue: Issue,
}

#[derive(Serialize)]
struct NextJsonOutput {
    resolved_issue_id: Option<String>,
    next: Option<Issue>,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run() -> uncle_funkle::Result<()> {
    let cli = Cli::parse();
    let engine = UncleFunkle::new(Config::default());

    match cli.command {
        Command::Scan { path } => {
            let mut state = engine.load_state(&path).await?;
            let report = engine.scan(&path).await?;
            let summary = engine.merge_scan(&mut state, report);
            let next = full_next_item_state(&engine, &state, None);
            engine.save_state(&path, &state).await?;
            render_summary(summary, next)?;
        }
        Command::Status { path, issue_id } => {
            let mut state = engine.load_state(&path).await?;
            if let Some(issue_id) = issue_id {
                uncle_funkle::recompute_scores(&mut state);
                let issue = state.issues.get(&issue_id).cloned().ok_or_else(|| {
                    UncleFunkleError::InvalidState(format!("issue not found: {issue_id}"))
                })?;
                render_issue(issue)?;
            } else {
                let summary = summary_from_state(&mut state);
                let next = full_next_item_state(&engine, &state, None);
                render_summary(summary, next)?;
            }
        }
        Command::List { path, all } => {
            let mut state = engine.load_state(&path).await?;
            uncle_funkle::recompute_scores(&mut state);
            let issues = sorted_issues(&engine, &state, all);
            render_list(issues)?;
        }
        Command::Next { path } => {
            let mut state = engine.load_state(&path).await?;
            let current = full_next_item_state(&engine, &state, None);
            let resolved_issue_id = current.as_ref().map(|item| item.id.clone());

            if let Some(issue_id) = resolved_issue_id.as_deref() {
                if !engine.resolve_issue(&mut state, issue_id, None) {
                    return Err(UncleFunkleError::InvalidState(format!(
                        "issue not found: {issue_id}"
                    )));
                }
                engine.save_state(&path, &state).await?;
            }

            let next = full_next_item_state(&engine, &state, None);
            render_next(resolved_issue_id, next)?;
        }
        Command::Resolve { issue_id, path } => {
            mutate_issue(
                &engine,
                path_or_current(path),
                &issue_id,
                |engine, state, id| engine.resolve_issue(state, id, None),
            )
            .await?;
        }
        Command::Defer { issue_id, path } => {
            mutate_issue(
                &engine,
                path_or_current(path),
                &issue_id,
                |engine, state, id| engine.defer_issue(state, id, None),
            )
            .await?;
        }
        Command::Dismiss { issue_id, path } => {
            mutate_issue(
                &engine,
                path_or_current(path),
                &issue_id,
                |engine, state, id| engine.dismiss_issue(state, id, None),
            )
            .await?;
        }
        Command::Reopen { issue_id, path } => {
            mutate_issue(
                &engine,
                path_or_current(path),
                &issue_id,
                |engine, state, id| engine.reopen_issue(state, id, None),
            )
            .await?;
        }
    }

    Ok(())
}

async fn mutate_issue<F>(
    engine: &UncleFunkle,
    path: PathBuf,
    issue_id: &str,
    action: F,
) -> uncle_funkle::Result<()>
where
    F: FnOnce(&UncleFunkle, &mut State, &str) -> bool,
{
    let mut state = engine.load_state(&path).await?;
    if !action(engine, &mut state, issue_id) {
        return Err(UncleFunkleError::InvalidState(format!(
            "issue not found: {issue_id}"
        )));
    }

    engine.save_state(&path, &state).await?;
    let issue =
        state.issues.get(issue_id).cloned().ok_or_else(|| {
            UncleFunkleError::InvalidState(format!("issue not found: {issue_id}"))
        })?;
    render_issue(issue)
}

fn render_summary(summary: MergeSummary, next: Option<Issue>) -> uncle_funkle::Result<()> {
    let output = SummaryJsonOutput {
        open_issues: summary.total_open,
        added: summary.added,
        updated: summary.updated,
        reopened: summary.reopened,
        auto_resolved: summary.auto_resolved,
        scores: summary.scores,
        next,
    };
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn render_list(issues: Vec<Issue>) -> uncle_funkle::Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&ListJsonOutput { issues })?
    );
    Ok(())
}

fn render_issue(issue: Issue) -> uncle_funkle::Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&IssueJsonOutput { issue })?
    );

    Ok(())
}

fn render_next(resolved_issue_id: Option<String>, next: Option<Issue>) -> uncle_funkle::Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&NextJsonOutput {
            resolved_issue_id,
            next,
        })?
    );
    Ok(())
}

fn full_next_item_state(
    engine: &UncleFunkle,
    state: &State,
    selected_issue_id: Option<&str>,
) -> Option<Issue> {
    match selected_issue_id {
        Some(issue_id) => state.issues.get(issue_id).cloned(),
        None => {
            let item = engine.next(state)?;
            let selected_issue = item
                .issue_ids
                .iter()
                .find_map(|issue_id| state.issues.get(issue_id))?
                .clone();
            Some(selected_issue)
        }
    }
}

fn sorted_issues(engine: &UncleFunkle, state: &State, all: bool) -> Vec<Issue> {
    let plan = engine.plan(state);
    let priorities = plan
        .items
        .into_iter()
        .flat_map(|item| {
            item.issue_ids
                .into_iter()
                .map(move |issue_id| (issue_id, item.priority))
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    let mut issues: Vec<Issue> = state
        .issues
        .values()
        .filter(|issue| all || issue.status == IssueStatus::Open)
        .cloned()
        .collect();

    issues.sort_by_key(|issue| {
        (
            status_rank(issue.status),
            Reverse(*priorities.get(&issue.id).unwrap_or(&0)),
            Reverse(issue.tier.weight()),
            issue.path.clone(),
            issue.id.clone(),
        )
    });

    issues
}

fn summary_from_state(state: &mut State) -> MergeSummary {
    let scores = uncle_funkle::recompute_scores(state);
    MergeSummary {
        total_open: state.stats.open_issues,
        scores,
        ..MergeSummary::default()
    }
}

fn status_rank(status: IssueStatus) -> u8 {
    match status {
        IssueStatus::Open => 0,
        IssueStatus::Deferred => 1,
        IssueStatus::Resolved => 2,
        IssueStatus::Dismissed => 3,
    }
}

fn path_or_current(path: Option<PathBuf>) -> PathBuf {
    path.unwrap_or_else(|| PathBuf::from("."))
}
