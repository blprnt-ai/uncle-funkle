use std::path::Path;

use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::model::LanguageKind;

pub fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

pub fn to_unix_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub fn normalize_whitespace(input: &str) -> String {
    let mut normalized = String::new();
    for (index, segment) in input.split_whitespace().enumerate() {
        if index > 0 {
            normalized.push(' ');
        }
        normalized.push_str(segment);
    }
    normalized
}

pub fn stable_hash<I, S>(parts: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut hasher = blake3::Hasher::new();
    for part in parts {
        hasher.update(part.as_ref().as_bytes());
        hasher.update(&[0x1f]);
    }
    hasher.finalize().to_hex().to_string()
}

pub fn stable_issue_id(fingerprint: &str) -> String {
    let digest = stable_hash([fingerprint]);
    format!("iss_{}", &digest[..16])
}

pub fn humanize_detector(detector: &str) -> String {
    let mut out = String::new();
    let raw = detector.replace(['.', '_'], " ");
    for (index, token) in raw.split_whitespace().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        let mut chars = token.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}

pub fn clamp_score(value: f32) -> f32 {
    value.clamp(0.0, 100.0)
}

pub fn round1(value: f32) -> f32 {
    (value * 10.0).round() / 10.0
}

pub fn language_from_path(path: &Path) -> LanguageKind {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());

    match extension.as_deref() {
        Some("rs") => LanguageKind::Rust,
        Some("py") => LanguageKind::Python,
        Some("ts") | Some("tsx") => LanguageKind::TypeScript,
        Some("js") | Some("jsx") | Some("mjs") | Some("cjs") => LanguageKind::JavaScript,
        Some("cs") => LanguageKind::CSharp,
        Some("go") => LanguageKind::Go,
        Some("dart") => LanguageKind::Dart,
        Some("gd") | Some("gdscript") => LanguageKind::Gdscript,
        Some("java") => LanguageKind::Java,
        Some("kt") | Some("kts") => LanguageKind::Kotlin,
        Some("rb") => LanguageKind::Ruby,
        Some("c") | Some("h") => LanguageKind::C,
        Some("cc") | Some("cpp") | Some("cxx") | Some("hpp") | Some("hh") => LanguageKind::Cpp,
        Some("swift") => LanguageKind::Swift,
        Some(other) => LanguageKind::Other(other.to_string()),
        None => LanguageKind::Other("unknown".to_string()),
    }
}
