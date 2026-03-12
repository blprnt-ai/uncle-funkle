use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::json;
use tokio::fs;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinSet;

use crate::config::Config;
use crate::error::{Result, UncleFunkleError};
use crate::model::{
    Confidence, FileSummary, Finding, IssueSource, LanguageKind, Location, ScanMetadata,
    ScanReport, Tier,
};
use crate::util::{
    language_from_path, normalize_whitespace, now_rfc3339, stable_hash, stable_issue_id,
    to_unix_path,
};

static TODO_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(TODO|FIXME|HACK|XXX)\b[:\-\s]*(.*)$").expect("valid TODO regex")
});
static RUST_FN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:const\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)",
    )
    .expect("valid rust fn regex")
});
static PY_FN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:async\s+def|def)\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(")
        .expect("valid python fn regex")
});
static GDSCRIPT_FN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*func\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(").expect("valid gdscript fn regex")
});
static GO_FN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*func\s*(?:\([^)]*\)\s*)?([A-Za-z_][A-Za-z0-9_]*)\s*\(")
        .expect("valid go fn regex")
});
static JS_FN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:export\s+)?(?:async\s+)?function\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*\(")
        .expect("valid js fn regex")
});
static GENERIC_BRACE_FN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:(?:public|private|protected|internal|static|final|virtual|override|sealed|abstract|partial|async|export)\s+)*(?:[A-Za-z_][A-Za-z0-9_<>\[\]\?,:&\s\*]+\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*\([^;]*\)\s*\{?\s*$")
        .expect("valid generic fn regex")
});
static BRANCH_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:if|match|for|while|switch|case|catch|elif|when)\b")
        .expect("valid branch regex")
});

#[derive(Debug, Clone)]
struct DuplicateLine {
    line_number: usize,
    normalized: String,
}

#[derive(Debug, Clone)]
struct DuplicateOccurrence {
    path: String,
    start_line: usize,
    end_line: usize,
    snippet: String,
}

#[derive(Debug, Clone)]
struct FunctionSpan {
    name: String,
    start_line: usize,
    end_line: usize,
    max_nesting: usize,
    branch_points: usize,
}

#[derive(Debug, Clone)]
struct FileAnalysis {
    summary: FileSummary,
    findings: Vec<Finding>,
    duplicate_lines: Vec<DuplicateLine>,
}

pub async fn scan_project(root: &Path, config: &Config) -> Result<ScanReport> {
    let generated_at = now_rfc3339();
    let discovered = discover_files(root, config).await?;
    let semaphore = Arc::new(Semaphore::new(config.max_concurrency.max(1)));
    let mut join_set = JoinSet::new();

    for path in discovered.iter().cloned() {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|error| UncleFunkleError::InvalidState(error.to_string()))?;
        let scan_root = root.to_path_buf();
        let scan_config = config.clone();
        join_set.spawn(async move { analyze_file(scan_root, path, scan_config, permit).await });
    }

    let mut analyses = Vec::with_capacity(discovered.len());
    while let Some(joined) = join_set.join_next().await {
        let analysis = joined.map_err(|error| UncleFunkleError::Join(error.to_string()))??;
        analyses.push(analysis);
    }

    analyses.sort_by(|left, right| left.summary.path.cmp(&right.summary.path));

    let mut findings = Vec::new();
    let mut files = Vec::new();
    let mut scanned_files = 0usize;
    let mut skipped_files = 0usize;
    let mut total_lines = 0usize;
    let mut total_bytes = 0u64;

    for analysis in &analyses {
        if analysis.summary.skipped {
            skipped_files += 1;
        } else {
            scanned_files += 1;
        }
        total_lines += analysis.summary.total_lines;
        total_bytes += analysis.summary.bytes;
        files.push(analysis.summary.clone());
        findings.extend(analysis.findings.clone());
    }

    findings.extend(detect_duplicate_blocks(&analyses, config));

    let metadata = ScanMetadata {
        root: to_unix_path(root),
        generated_at: generated_at.clone(),
        candidate_files: discovered.len(),
        scanned_files,
        skipped_files,
        total_lines,
        total_bytes,
    };

    Ok(ScanReport {
        root: metadata.root.clone(),
        generated_at,
        files,
        findings,
        metadata,
    })
}

async fn discover_files(root: &Path, config: &Config) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut queue = VecDeque::new();
    queue.push_back(root.to_path_buf());

    while let Some(dir) = queue.pop_front() {
        let mut entries = fs::read_dir(&dir)
            .await
            .map_err(|error| UncleFunkleError::io(&dir, error))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|error| UncleFunkleError::io(&dir, error))?
        {
            let path = entry.path();
            let file_type = entry
                .file_type()
                .await
                .map_err(|error| UncleFunkleError::io(&path, error))?;

            if file_type.is_dir() {
                let should_skip = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| config.should_skip_dir_name(name))
                    .unwrap_or(false)
                    || config.should_skip_path(&path);
                if !should_skip {
                    queue.push_back(path);
                }
                continue;
            }

            if file_type.is_file()
                && config.extension_in_scope(&path)
                && !config.should_skip_path(&path)
            {
                files.push(path);
            }
        }
    }

    files.sort();
    Ok(files)
}

async fn analyze_file(
    root: PathBuf,
    path: PathBuf,
    config: Config,
    _permit: OwnedSemaphorePermit,
) -> Result<FileAnalysis> {
    let bytes = fs::read(&path)
        .await
        .map_err(|error| UncleFunkleError::io(&path, error))?;

    let language = language_from_path(&path);
    let rendered = path.strip_prefix(&root).unwrap_or(&path);
    let relative_path = to_unix_path(rendered);

    if bytes.contains(&0) || bytes.len() > config.max_file_bytes {
        return Ok(FileAnalysis {
            summary: FileSummary {
                path: relative_path,
                language,
                bytes: bytes.len() as u64,
                total_lines: 0,
                non_empty_lines: 0,
                skipped: true,
            },
            findings: Vec::new(),
            duplicate_lines: Vec::new(),
        });
    }

    let content = String::from_utf8_lossy(&bytes).into_owned();
    let lines: Vec<String> = content.lines().map(str::to_string).collect();
    let non_empty_lines = lines.iter().filter(|line| !line.trim().is_empty()).count();

    let mut findings = Vec::new();
    detect_todo_comments(&mut findings, &language, &relative_path, &lines);
    if config.detect_debug_artifacts {
        detect_debug_artifacts(&mut findings, &language, &relative_path, &lines);
    }
    detect_large_file(&mut findings, &language, &relative_path, &lines, &config);
    detect_long_lines(&mut findings, &language, &relative_path, &lines, &config);

    let functions = extract_functions(&language, &lines);
    detect_long_functions(
        &mut findings,
        &language,
        &relative_path,
        &functions,
        &config,
    );
    detect_deep_nesting(
        &mut findings,
        &language,
        &relative_path,
        &functions,
        &config,
    );
    detect_branch_density(
        &mut findings,
        &language,
        &relative_path,
        &functions,
        &config,
    );

    Ok(FileAnalysis {
        summary: FileSummary {
            path: relative_path.clone(),
            language: language.clone(),
            bytes: bytes.len() as u64,
            total_lines: lines.len(),
            non_empty_lines,
            skipped: false,
        },
        findings,
        duplicate_lines: duplicate_lines(&language, &lines),
    })
}

fn detect_todo_comments(
    findings: &mut Vec<Finding>,
    language: &LanguageKind,
    path: &str,
    lines: &[String],
) {
    for (index, line) in lines.iter().enumerate() {
        let comment_text = comment_text(language, line);
        let Some(comment_text) = comment_text else {
            continue;
        };
        let Some(captures) = TODO_RE.captures(comment_text) else {
            continue;
        };

        let token = captures
            .get(1)
            .map(|capture| capture.as_str().to_ascii_uppercase())
            .unwrap_or_else(|| "TODO".to_string());
        let message = captures
            .get(2)
            .map(|capture| capture.as_str().trim().to_string())
            .unwrap_or_default();

        let key = format!(
            "{}:{}:{}",
            token,
            index + 1,
            normalize_whitespace(comment_text)
        );
        let fingerprint = stable_hash(["todo_comment", path, &key]);
        let summary = if message.is_empty() {
            format!("{} comment left in code", token)
        } else {
            format!("{} comment left in code: {}", token, message)
        };

        findings.push(Finding {
            id: stable_issue_id(&fingerprint),
            fingerprint,
            detector: "todo_comment".to_string(),
            source: IssueSource::Mechanical,
            language: language.clone(),
            tier: Tier::T2,
            confidence: Confidence::High,
            path: path.to_string(),
            summary,
            description: "Remove the placeholder comment or convert it into completed code or a tracked follow-up item."
                .to_string(),
            location: Location {
                line_start: Some(index + 1),
                line_end: Some(index + 1),
            },
            detail: BTreeMap::from([
                ("token".to_string(), json!(token)),
                ("text".to_string(), json!(comment_text.trim())),
            ]),
        });
    }
}

fn detect_debug_artifacts(
    findings: &mut Vec<Finding>,
    language: &LanguageKind,
    path: &str,
    lines: &[String],
) {
    let patterns = debug_patterns(language);
    if patterns.is_empty() {
        return;
    }

    for (index, line) in lines.iter().enumerate() {
        let code = code_portion(language, line);
        if code.trim().is_empty() {
            continue;
        }

        if let Some(pattern) = patterns.iter().find(|pattern| code.contains(**pattern)) {
            let key = format!("{}:{}:{}", pattern, index + 1, normalize_whitespace(code));
            let fingerprint = stable_hash(["debug_artifact", path, &key]);
            findings.push(Finding {
                id: stable_issue_id(&fingerprint),
                fingerprint,
                detector: "debug_artifact".to_string(),
                source: IssueSource::Mechanical,
                language: language.clone(),
                tier: Tier::T2,
                confidence: Confidence::Medium,
                path: path.to_string(),
                summary: format!("Debug artifact found: {}", pattern),
                description:
                    "Remove one-off debugging output or move it behind an intentional logging or tracing boundary."
                        .to_string(),
                location: Location {
                    line_start: Some(index + 1),
                    line_end: Some(index + 1),
                },
                detail: BTreeMap::from([(String::from("pattern"), json!(pattern))]),
            });
        }
    }
}

fn detect_large_file(
    findings: &mut Vec<Finding>,
    language: &LanguageKind,
    path: &str,
    lines: &[String],
    config: &Config,
) {
    let threshold = config.thresholds.large_file_lines;
    if lines.len() <= threshold {
        return;
    }

    let tier = if lines.len() > threshold * 2 {
        Tier::T3
    } else {
        Tier::T2
    };
    let fingerprint = stable_hash(["large_file", path]);
    findings.push(Finding {
        id: stable_issue_id(&fingerprint),
        fingerprint,
        detector: "large_file".to_string(),
        source: IssueSource::Mechanical,
        language: language.clone(),
        tier,
        confidence: Confidence::High,
        path: path.to_string(),
        summary: format!(
            "Large file: {} lines exceeds threshold of {}",
            lines.len(),
            threshold
        ),
        description:
            "Split the file by responsibility and move cohesive behavior into smaller modules or types."
                .to_string(),
        location: Location {
            line_start: Some(1),
            line_end: Some(lines.len()),
        },
        detail: BTreeMap::from([
            (String::from("line_count"), json!(lines.len())),
            (String::from("threshold"), json!(threshold)),
        ]),
    });
}

fn detect_long_lines(
    findings: &mut Vec<Finding>,
    language: &LanguageKind,
    path: &str,
    lines: &[String],
    config: &Config,
) {
    let long_lines: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            (line.chars().count() > config.thresholds.long_line).then_some(index + 1)
        })
        .collect();

    if long_lines.is_empty() {
        return;
    }

    let fingerprint = stable_hash(["long_line", path]);
    findings.push(Finding {
        id: stable_issue_id(&fingerprint),
        fingerprint,
        detector: "long_line".to_string(),
        source: IssueSource::Mechanical,
        language: language.clone(),
        tier: Tier::T1,
        confidence: Confidence::High,
        path: path.to_string(),
        summary: format!(
            "{} line(s) exceed max length of {}",
            long_lines.len(),
            config.thresholds.long_line
        ),
        description:
            "Wrap or restructure the statement so each line stays readable during review and maintenance."
                .to_string(),
        location: Location {
            line_start: long_lines.first().copied(),
            line_end: long_lines.last().copied(),
        },
        detail: BTreeMap::from([
            (String::from("lines"), json!(long_lines)),
            (
                String::from("threshold"),
                json!(config.thresholds.long_line),
            ),
        ]),
    });
}

fn detect_long_functions(
    findings: &mut Vec<Finding>,
    language: &LanguageKind,
    path: &str,
    functions: &[FunctionSpan],
    config: &Config,
) {
    for function in functions {
        let line_count = function.end_line.saturating_sub(function.start_line) + 1;
        if line_count <= config.thresholds.long_function_lines {
            continue;
        }

        let tier = if line_count > config.thresholds.long_function_lines * 2 {
            Tier::T3
        } else {
            Tier::T2
        };
        let key = format!("{}:{}", function.name, function.start_line);
        let fingerprint = stable_hash(["long_function", path, &key]);
        findings.push(Finding {
            id: stable_issue_id(&fingerprint),
            fingerprint,
            detector: "long_function".to_string(),
            source: IssueSource::Mechanical,
            language: language.clone(),
            tier,
            confidence: Confidence::Medium,
            path: path.to_string(),
            summary: format!(
                "Function `{}` spans {} lines",
                function.name, line_count
            ),
            description:
                "Break the function into named helpers so each unit holds one responsibility and can be tested in isolation."
                    .to_string(),
            location: Location {
                line_start: Some(function.start_line),
                line_end: Some(function.end_line),
            },
            detail: BTreeMap::from([
                (String::from("function"), json!(function.name.clone())),
                (String::from("line_count"), json!(line_count)),
                (
                    String::from("threshold"),
                    json!(config.thresholds.long_function_lines),
                ),
            ]),
        });
    }
}

fn detect_deep_nesting(
    findings: &mut Vec<Finding>,
    language: &LanguageKind,
    path: &str,
    functions: &[FunctionSpan],
    config: &Config,
) {
    for function in functions {
        if function.max_nesting <= config.thresholds.deep_nesting_depth {
            continue;
        }

        let tier = if function.max_nesting > config.thresholds.deep_nesting_depth + 2 {
            Tier::T4
        } else {
            Tier::T3
        };
        let key = format!("{}:{}", function.name, function.start_line);
        let fingerprint = stable_hash(["deep_nesting", path, &key]);
        findings.push(Finding {
            id: stable_issue_id(&fingerprint),
            fingerprint,
            detector: "deep_nesting".to_string(),
            source: IssueSource::Mechanical,
            language: language.clone(),
            tier,
            confidence: Confidence::Medium,
            path: path.to_string(),
            summary: format!(
                "Function `{}` reaches nesting depth {}",
                function.name, function.max_nesting
            ),
            description:
                "Flatten the control flow with guard clauses or helper functions so the main path stays obvious."
                    .to_string(),
            location: Location {
                line_start: Some(function.start_line),
                line_end: Some(function.end_line),
            },
            detail: BTreeMap::from([
                (String::from("function"), json!(function.name.clone())),
                (String::from("max_nesting"), json!(function.max_nesting)),
                (
                    String::from("threshold"),
                    json!(config.thresholds.deep_nesting_depth),
                ),
            ]),
        });
    }
}

fn detect_branch_density(
    findings: &mut Vec<Finding>,
    language: &LanguageKind,
    path: &str,
    functions: &[FunctionSpan],
    config: &Config,
) {
    for function in functions {
        if function.branch_points <= config.thresholds.branch_points_per_function {
            continue;
        }

        let tier = if function.branch_points > config.thresholds.branch_points_per_function * 2 {
            Tier::T3
        } else {
            Tier::T2
        };
        let key = format!("{}:{}", function.name, function.start_line);
        let fingerprint = stable_hash(["branch_density", path, &key]);
        findings.push(Finding {
            id: stable_issue_id(&fingerprint),
            fingerprint,
            detector: "branch_density".to_string(),
            source: IssueSource::Mechanical,
            language: language.clone(),
            tier,
            confidence: Confidence::Medium,
            path: path.to_string(),
            summary: format!(
                "Function `{}` has {} branch points",
                function.name, function.branch_points
            ),
            description:
                "Separate decision logic from execution steps or use smaller helper functions to reduce branching pressure."
                    .to_string(),
            location: Location {
                line_start: Some(function.start_line),
                line_end: Some(function.end_line),
            },
            detail: BTreeMap::from([
                (String::from("function"), json!(function.name.clone())),
                (
                    String::from("branch_points"),
                    json!(function.branch_points),
                ),
                (
                    String::from("threshold"),
                    json!(config.thresholds.branch_points_per_function),
                ),
            ]),
        });
    }
}

fn detect_duplicate_blocks(analyses: &[FileAnalysis], config: &Config) -> Vec<Finding> {
    let window = config.thresholds.duplicate_window_lines.max(3);
    let mut grouped: BTreeMap<String, Vec<DuplicateOccurrence>> = BTreeMap::new();

    for analysis in analyses {
        if analysis.summary.skipped || analysis.duplicate_lines.len() < window {
            continue;
        }

        for slice in analysis.duplicate_lines.windows(window) {
            let snippet = slice
                .iter()
                .map(|line| line.normalized.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if snippet.is_empty() {
                continue;
            }

            let signature = stable_hash([snippet.as_str()]);
            grouped
                .entry(signature)
                .or_default()
                .push(DuplicateOccurrence {
                    path: analysis.summary.path.clone(),
                    start_line: slice.first().map(|line| line.line_number).unwrap_or(1),
                    end_line: slice.last().map(|line| line.line_number).unwrap_or(1),
                    snippet,
                });
        }
    }

    let mut ranked: Vec<(String, Vec<DuplicateOccurrence>)> = grouped
        .into_iter()
        .filter_map(|(signature, occurrences)| {
            let deduped = dedupe_duplicate_occurrences(occurrences);
            (deduped.len() >= config.thresholds.duplicate_min_occurrences)
                .then_some((signature, deduped))
        })
        .collect();

    ranked.sort_by_key(|item| std::cmp::Reverse(item.1.len()));
    ranked.truncate(config.thresholds.max_duplicate_reports);

    ranked
        .into_iter()
        .map(|(signature, occurrences)| {
            let first = occurrences.first().cloned().unwrap_or(DuplicateOccurrence {
                path: String::new(),
                start_line: 1,
                end_line: 1,
                snippet: String::new(),
            });
            let paths = occurrences
                .iter()
                .map(|occurrence| format!("{}:{}-{}", occurrence.path, occurrence.start_line, occurrence.end_line))
                .collect::<Vec<_>>()
                .join("|");
            let fingerprint = stable_hash(["duplicate_block", &signature, &paths]);
            let tier = if occurrences.len() >= 3 { Tier::T3 } else { Tier::T2 };
            let detail = BTreeMap::from([
                (String::from("signature"), json!(signature)),
                (
                    String::from("occurrences"),
                    json!(occurrences
                        .iter()
                        .map(|occurrence| {
                            json!({
                                "path": occurrence.path.clone(),
                                "line_start": occurrence.start_line,
                                "line_end": occurrence.end_line,
                                "snippet": occurrence.snippet.clone(),
                            })
                        })
                        .collect::<Vec<_>>()),
                ),
            ]);

            Finding {
                id: stable_issue_id(&fingerprint),
                fingerprint,
                detector: "duplicate_block".to_string(),
                source: IssueSource::Mechanical,
                language: LanguageKind::Other("multi".to_string()),
                tier,
                confidence: Confidence::Medium,
                path: first.path,
                summary: format!(
                    "Duplicate code block appears in {} locations",
                    occurrences.len()
                ),
                description:
                    "Extract the repeated block into a shared abstraction so each site expresses only its local intent."
                        .to_string(),
                location: Location {
                    line_start: Some(first.start_line),
                    line_end: Some(first.end_line),
                },
                detail,
            }
        })
        .collect()
}

fn extract_functions(language: &LanguageKind, lines: &[String]) -> Vec<FunctionSpan> {
    match language {
        LanguageKind::Python | LanguageKind::Gdscript => {
            extract_indentation_functions(language, lines)
        }
        LanguageKind::Ruby => Vec::new(),
        _ => extract_brace_functions(language, lines),
    }
}

fn extract_brace_functions(language: &LanguageKind, lines: &[String]) -> Vec<FunctionSpan> {
    let mut spans = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        let Some(name) = function_name(language, &lines[index]) else {
            index += 1;
            continue;
        };

        let mut brace_index = None;
        for (probe, line) in lines
            .iter()
            .enumerate()
            .take(lines.len().min(index + 4))
            .skip(index)
        {
            if line.contains('{') {
                brace_index = Some(probe);
                break;
            }
            if line.trim_end().ends_with(';') {
                break;
            }
        }

        let Some(start_brace_index) = brace_index else {
            index += 1;
            continue;
        };

        let mut balance = 0isize;
        let mut max_balance = 0isize;
        let mut branch_points = 0usize;
        let mut found_body = false;
        let mut end = start_brace_index;

        for (probe, line) in lines.iter().enumerate().skip(index) {
            let code = code_portion(language, line);
            branch_points += count_branch_points(code);
            let delta = brace_delta(code);
            if delta > 0 {
                found_body = true;
            }
            balance += delta;
            if found_body {
                max_balance = max_balance.max(balance);
            }
            if found_body && balance <= 0 {
                end = probe;
                break;
            }
        }

        if !found_body {
            index += 1;
            continue;
        }

        let max_nesting = if max_balance > 1 {
            (max_balance - 1) as usize
        } else {
            0
        };

        spans.push(FunctionSpan {
            name,
            start_line: index + 1,
            end_line: end + 1,
            max_nesting,
            branch_points,
        });
        index = end.saturating_add(1);
    }

    spans
}

fn extract_indentation_functions(language: &LanguageKind, lines: &[String]) -> Vec<FunctionSpan> {
    let mut spans = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        let Some(name) = function_name(language, &lines[index]) else {
            index += 1;
            continue;
        };

        let base_indent = indentation(&lines[index]);
        let mut end = index;
        let mut max_indent = base_indent;
        let mut branch_points = 0usize;
        let mut probe = index + 1;

        while probe < lines.len() {
            let trimmed = lines[probe].trim();
            if !trimmed.is_empty() && indentation(&lines[probe]) <= base_indent {
                break;
            }
            if !trimmed.is_empty() {
                max_indent = max_indent.max(indentation(&lines[probe]));
                branch_points += count_branch_points(code_portion(language, &lines[probe]));
            }
            end = probe;
            probe += 1;
        }

        spans.push(FunctionSpan {
            name,
            start_line: index + 1,
            end_line: end + 1,
            max_nesting: max_indent.saturating_sub(base_indent) / 4,
            branch_points,
        });
        index = end.saturating_add(1);
    }

    spans
}

fn function_name(language: &LanguageKind, line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || looks_like_control_flow(trimmed) {
        return None;
    }

    let captures = match language {
        LanguageKind::Rust => RUST_FN_RE.captures(trimmed),
        LanguageKind::Python => PY_FN_RE.captures(trimmed),
        LanguageKind::Gdscript => GDSCRIPT_FN_RE.captures(trimmed),
        LanguageKind::Go => GO_FN_RE.captures(trimmed),
        LanguageKind::JavaScript | LanguageKind::TypeScript => JS_FN_RE.captures(trimmed),
        LanguageKind::Ruby => None,
        _ => GENERIC_BRACE_FN_RE.captures(trimmed),
    };

    captures.and_then(|captures| captures.get(1).map(|capture| capture.as_str().to_string()))
}

fn duplicate_lines(language: &LanguageKind, lines: &[String]) -> Vec<DuplicateLine> {
    let mut out = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        let code = code_portion(language, line).trim();
        if code.is_empty() {
            continue;
        }

        let normalized = normalize_whitespace(code);
        if normalized.len() < 3 {
            continue;
        }
        if matches!(
            normalized.as_str(),
            "{" | "}" | "(" | ")" | "[" | "]" | "end" | "else"
        ) {
            continue;
        }
        if normalized
            .chars()
            .all(|character| "{}[]();,.".contains(character))
        {
            continue;
        }

        out.push(DuplicateLine {
            line_number: index + 1,
            normalized,
        });
    }

    out
}

fn dedupe_duplicate_occurrences(
    mut occurrences: Vec<DuplicateOccurrence>,
) -> Vec<DuplicateOccurrence> {
    occurrences.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.start_line.cmp(&right.start_line))
            .then_with(|| left.end_line.cmp(&right.end_line))
    });

    let mut deduped = Vec::new();
    for occurrence in occurrences {
        let should_skip = deduped
            .last()
            .is_some_and(|previous: &DuplicateOccurrence| {
                previous.path == occurrence.path && occurrence.start_line <= previous.end_line
            });
        if !should_skip {
            deduped.push(occurrence);
        }
    }

    deduped
}

fn count_branch_points(line: &str) -> usize {
    BRANCH_RE.find_iter(line).count()
}

fn brace_delta(line: &str) -> isize {
    let mut delta = 0isize;
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;

    for character in line.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' if in_single || in_double => {
                escaped = true;
            }
            '\'' if !in_double => {
                in_single = !in_single;
            }
            '"' if !in_single => {
                in_double = !in_double;
            }
            '{' if !in_single && !in_double => {
                delta += 1;
            }
            '}' if !in_single && !in_double => {
                delta -= 1;
            }
            _ => {}
        }
    }

    delta
}

fn indentation(line: &str) -> usize {
    line.chars()
        .take_while(|character| character.is_whitespace())
        .map(|character| if character == '\t' { 4 } else { 1 })
        .sum()
}

fn comment_text<'a>(language: &LanguageKind, line: &'a str) -> Option<&'a str> {
    let trimmed = line.trim_start();
    match language {
        LanguageKind::Rust
        | LanguageKind::JavaScript
        | LanguageKind::TypeScript
        | LanguageKind::CSharp
        | LanguageKind::Go
        | LanguageKind::Dart
        | LanguageKind::Java
        | LanguageKind::Kotlin
        | LanguageKind::C
        | LanguageKind::Cpp
        | LanguageKind::Swift
        | LanguageKind::Other(_) => trimmed.strip_prefix("//"),
        LanguageKind::Python | LanguageKind::Gdscript | LanguageKind::Ruby => {
            trimmed.strip_prefix('#')
        }
    }
}

fn code_portion<'a>(language: &LanguageKind, line: &'a str) -> &'a str {
    let trimmed = line.trim_start();
    match language {
        LanguageKind::Rust
        | LanguageKind::JavaScript
        | LanguageKind::TypeScript
        | LanguageKind::CSharp
        | LanguageKind::Go
        | LanguageKind::Dart
        | LanguageKind::Java
        | LanguageKind::Kotlin
        | LanguageKind::C
        | LanguageKind::Cpp
        | LanguageKind::Swift
        | LanguageKind::Other(_) => trimmed.split("//").next().unwrap_or(trimmed),
        LanguageKind::Python | LanguageKind::Gdscript | LanguageKind::Ruby => {
            if trimmed.starts_with('#') {
                ""
            } else {
                trimmed.split('#').next().unwrap_or(trimmed)
            }
        }
    }
}

fn debug_patterns(language: &LanguageKind) -> &'static [&'static str] {
    match language {
        LanguageKind::Rust => &["dbg!", "println!", "eprintln!"],
        LanguageKind::JavaScript | LanguageKind::TypeScript => {
            &["console.log", "console.debug", "debugger;"]
        }
        LanguageKind::Python | LanguageKind::Gdscript => &["print(", "breakpoint("],
        LanguageKind::Go => &["fmt.Println", "fmt.Printf"],
        LanguageKind::CSharp => &["Console.WriteLine"],
        LanguageKind::Java => &["System.out.println"],
        _ => &[],
    }
}

fn looks_like_control_flow(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    [
        "if ", "if(", "for ", "for(", "while ", "while(", "switch ", "switch(", "match ", "catch ",
        "else ",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}
