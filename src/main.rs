use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde::Serialize;
use uncle_funkle::{Config, UncleFunkle};

#[derive(Parser, Debug)]
#[command(name = "uncle-funkle")]
#[command(about = "Scan a codebase or inspect saved state")]
struct Cli {
    #[arg(long)]
    json: bool,

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
    },
}

#[derive(Serialize)]
struct JsonOutput {
    open_issues: usize,
    added: usize,
    updated: usize,
    reopened: usize,
    auto_resolved: usize,
    scores: uncle_funkle::ScoreSnapshot,
    next: Option<String>,
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

    let (summary, next) = match cli.command {
        Command::Scan { path } => {
            let mut state = engine.load_state(&path).await?;
            let report = engine.scan(&path).await?;
            let summary = engine.merge_scan(&mut state, report);
            let next = engine.next(&state);
            engine.save_state(&path, &state).await?;
            (summary, next)
        }
        Command::Status { path } => {
            let mut state = engine.load_state(&path).await?;
            let summary = uncle_funkle::recompute_scores(&mut state);
            let next = engine.next(&state);
            (
                uncle_funkle::MergeSummary {
                    total_open: state.stats.open_issues,
                    scores: summary,
                    ..uncle_funkle::MergeSummary::default()
                },
                next,
            )
        }
    };

    if cli.json {
        let output = JsonOutput {
            open_issues: summary.total_open,
            added: summary.added,
            updated: summary.updated,
            reopened: summary.reopened,
            auto_resolved: summary.auto_resolved,
            scores: summary.scores,
            next: next.map(|item| item.title),
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!("open issues: {}", summary.total_open);
    println!(
        "changes: +{} updated:{} reopened:{} auto-resolved:{}",
        summary.added, summary.updated, summary.reopened, summary.auto_resolved
    );
    println!(
        "scores: overall {:.1} objective {:.1} strict {:.1} verified {:.1}",
        summary.scores.overall,
        summary.scores.objective,
        summary.scores.strict,
        summary.scores.verified
    );

    if let Some(item) = next {
        println!("next: {}", item.title);
    } else {
        println!("next: none");
    }

    Ok(())
}
