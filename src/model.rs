use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::util::now_rfc3339;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum Tier {
    #[serde(rename = "T1")]
    T1,
    #[serde(rename = "T2")]
    #[default]
    T2,
    #[serde(rename = "T3")]
    T3,
    #[serde(rename = "T4")]
    T4,
}

impl Tier {
    pub fn weight(self) -> u32 {
        match self {
            Self::T1 => 1,
            Self::T2 => 2,
            Self::T3 => 4,
            Self::T4 => 7,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::T1 => "T1",
            Self::T2 => "T2",
            Self::T3 => "T3",
            Self::T4 => "T4",
        }
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum Confidence {
    Low,
    #[default]
    Medium,
    High,
}

impl Confidence {
    pub fn multiplier(self) -> f32 {
        match self {
            Self::Low => 0.8,
            Self::Medium => 1.0,
            Self::High => 1.25,
        }
    }

    pub fn priority_points(self) -> u32 {
        match self {
            Self::Low => 5,
            Self::Medium => 10,
            Self::High => 15,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum IssueStatus {
    #[default]
    Open,
    Resolved,
    Deferred,
    Dismissed,
}

impl IssueStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Resolved => "resolved",
            Self::Deferred => "deferred",
            Self::Dismissed => "dismissed",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum IssueSource {
    #[default]
    Mechanical,
    Subjective,
}

impl IssueSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mechanical => "mechanical",
            Self::Subjective => "subjective",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum LanguageKind {
    Rust,
    Python,
    TypeScript,
    JavaScript,
    CSharp,
    Go,
    Dart,
    Gdscript,
    Java,
    Kotlin,
    Ruby,
    C,
    Cpp,
    Swift,
    Other(String),
}

impl LanguageKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::CSharp => "csharp",
            Self::Go => "go",
            Self::Dart => "dart",
            Self::Gdscript => "gdscript",
            Self::Java => "java",
            Self::Kotlin => "kotlin",
            Self::Ruby => "ruby",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Swift => "swift",
            Self::Other(other) => other.as_str(),
        }
    }
}

impl Default for LanguageKind {
    fn default() -> Self {
        Self::Other("unknown".to_string())
    }
}

impl fmt::Display for LanguageKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct Location {
    pub line_start: Option<usize>,
    pub line_end: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Finding {
    pub id: String,
    pub fingerprint: String,
    pub detector: String,
    pub source: IssueSource,
    pub language: LanguageKind,
    pub tier: Tier,
    pub confidence: Confidence,
    pub path: String,
    pub summary: String,
    pub description: String,
    pub location: Location,
    pub detail: BTreeMap<String, Value>,
}

impl Default for Finding {
    fn default() -> Self {
        Self {
            id: String::new(),
            fingerprint: String::new(),
            detector: String::new(),
            source: IssueSource::Mechanical,
            language: LanguageKind::default(),
            tier: Tier::default(),
            confidence: Confidence::default(),
            path: String::new(),
            summary: String::new(),
            description: String::new(),
            location: Location::default(),
            detail: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Issue {
    pub id: String,
    pub fingerprint: String,
    pub detector: String,
    pub source: IssueSource,
    pub language: LanguageKind,
    pub tier: Tier,
    pub confidence: Confidence,
    pub path: String,
    pub summary: String,
    pub description: String,
    pub location: Location,
    pub detail: BTreeMap<String, Value>,
    pub status: IssueStatus,
    pub note: Option<String>,
    pub first_seen: String,
    pub last_seen: String,
    pub resolved_at: Option<String>,
    pub reopen_count: u32,
}

impl Default for Issue {
    fn default() -> Self {
        let now = now_rfc3339();
        Self {
            id: String::new(),
            fingerprint: String::new(),
            detector: String::new(),
            source: IssueSource::Mechanical,
            language: LanguageKind::default(),
            tier: Tier::default(),
            confidence: Confidence::default(),
            path: String::new(),
            summary: String::new(),
            description: String::new(),
            location: Location::default(),
            detail: BTreeMap::new(),
            status: IssueStatus::Open,
            note: None,
            first_seen: now.clone(),
            last_seen: now,
            resolved_at: None,
            reopen_count: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct FileSummary {
    pub path: String,
    pub language: LanguageKind,
    pub bytes: u64,
    pub total_lines: usize,
    pub non_empty_lines: usize,
    pub skipped: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ScanMetadata {
    pub root: String,
    pub generated_at: String,
    pub candidate_files: usize,
    pub scanned_files: usize,
    pub skipped_files: usize,
    pub total_lines: usize,
    pub total_bytes: u64,
}

impl Default for ScanMetadata {
    fn default() -> Self {
        Self {
            root: String::new(),
            generated_at: now_rfc3339(),
            candidate_files: 0,
            scanned_files: 0,
            skipped_files: 0,
            total_lines: 0,
            total_bytes: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct ScanReport {
    pub root: String,
    pub generated_at: String,
    pub files: Vec<FileSummary>,
    pub findings: Vec<Finding>,
    pub metadata: ScanMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SubjectiveFindingImport {
    pub path: Option<String>,
    pub summary: String,
    pub description: String,
    pub tier: Tier,
    pub confidence: Confidence,
    pub line_start: Option<usize>,
    pub line_end: Option<usize>,
    pub metadata: BTreeMap<String, Value>,
}

impl Default for SubjectiveFindingImport {
    fn default() -> Self {
        Self {
            path: None,
            summary: String::new(),
            description: String::new(),
            tier: Tier::T3,
            confidence: Confidence::Medium,
            line_start: None,
            line_end: None,
            metadata: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AssessmentImport {
    pub dimension: String,
    pub score: f32,
    pub summary: String,
    pub metadata: BTreeMap<String, Value>,
    pub findings: Vec<SubjectiveFindingImport>,
}

impl Default for AssessmentImport {
    fn default() -> Self {
        Self {
            dimension: String::new(),
            score: 100.0,
            summary: String::new(),
            metadata: BTreeMap::new(),
            findings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SubjectiveAssessment {
    pub dimension: String,
    pub score: f32,
    pub summary: String,
    pub imported_at: String,
    pub metadata: BTreeMap<String, Value>,
    pub finding_ids: Vec<String>,
}

impl Default for SubjectiveAssessment {
    fn default() -> Self {
        Self {
            dimension: String::new(),
            score: 100.0,
            summary: String::new(),
            imported_at: now_rfc3339(),
            metadata: BTreeMap::new(),
            finding_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Stats {
    pub total_issues: usize,
    pub open_issues: usize,
    pub resolved_issues: usize,
    pub deferred_issues: usize,
    pub dismissed_issues: usize,
    pub subjective_issues: usize,
    pub mechanical_issues: usize,
    pub candidate_files: usize,
    pub scanned_files: usize,
    pub skipped_files: usize,
    pub total_lines: usize,
    pub total_bytes: u64,
    pub issues_by_tier: BTreeMap<String, usize>,
    pub issues_by_detector: BTreeMap<String, usize>,
    pub issues_by_status: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct State {
    pub version: u32,
    pub created: String,
    pub last_scan: Option<String>,
    pub scan_count: u64,
    pub overall_score: f32,
    pub objective_score: f32,
    pub strict_score: f32,
    pub verified_strict_score: f32,
    pub stats: Stats,
    pub issues: BTreeMap<String, Issue>,
    pub scan_metadata: Option<ScanMetadata>,
    pub subjective_assessments: BTreeMap<String, SubjectiveAssessment>,
}

impl Default for State {
    fn default() -> Self {
        let now = now_rfc3339();
        Self {
            version: 1,
            created: now,
            last_scan: None,
            scan_count: 0,
            overall_score: 100.0,
            objective_score: 100.0,
            strict_score: 100.0,
            verified_strict_score: 100.0,
            stats: Stats::default(),
            issues: BTreeMap::new(),
            scan_metadata: None,
            subjective_assessments: BTreeMap::new(),
        }
    }
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PlanItem {
    pub key: String,
    pub title: String,
    pub summary: String,
    pub detector: String,
    pub file: Option<String>,
    pub tier: Tier,
    pub priority: u32,
    pub issue_ids: Vec<String>,
    pub issue_count: usize,
    pub guidance: String,
    pub resolve_hint: String,
}

impl Default for PlanItem {
    fn default() -> Self {
        Self {
            key: String::new(),
            title: String::new(),
            summary: String::new(),
            detector: String::new(),
            file: None,
            tier: Tier::T2,
            priority: 0,
            issue_ids: Vec::new(),
            issue_count: 0,
            guidance: String::new(),
            resolve_hint: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Plan {
    pub generated_at: String,
    pub items: Vec<PlanItem>,
}

impl Default for Plan {
    fn default() -> Self {
        Self {
            generated_at: now_rfc3339(),
            items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ScoreSnapshot {
    pub overall: f32,
    pub objective: f32,
    pub strict: f32,
    pub verified: f32,
}

impl Default for ScoreSnapshot {
    fn default() -> Self {
        Self {
            overall: 100.0,
            objective: 100.0,
            strict: 100.0,
            verified: 100.0,
        }
    }
}
