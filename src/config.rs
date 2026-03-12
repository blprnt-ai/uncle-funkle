use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub state_dir_name: String,
    pub state_file_name: String,
    pub include_extensions: BTreeSet<String>,
    pub exclude_dirs: BTreeSet<String>,
    pub exclude_suffixes: BTreeSet<String>,
    pub max_file_bytes: usize,
    pub max_concurrency: usize,
    pub detect_debug_artifacts: bool,
    pub thresholds: DetectorThresholds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DetectorThresholds {
    pub long_line: usize,
    pub large_file_lines: usize,
    pub long_function_lines: usize,
    pub deep_nesting_depth: usize,
    pub branch_points_per_function: usize,
    pub duplicate_window_lines: usize,
    pub duplicate_min_occurrences: usize,
    pub max_duplicate_reports: usize,
}

impl Default for Config {
    fn default() -> Self {
        let include_extensions = [
            "rs", "py", "ts", "tsx", "js", "jsx", "mjs", "cjs", "go", "cs", "dart", "gd",
            "gdscript", "java", "kt", "kts", "rb", "c", "h", "cc", "cpp", "cxx", "hpp", "hh",
            "swift",
        ]
        .into_iter()
        .map(str::to_string)
        .collect();

        let exclude_dirs = [
            ".git",
            ".hg",
            ".svn",
            ".idea",
            ".vscode",
            "node_modules",
            "dist",
            "build",
            "target",
            "coverage",
            ".venv",
            "venv",
            "vendor",
            ".uncle_funkle",
        ]
        .into_iter()
        .map(str::to_string)
        .collect();

        let exclude_suffixes = [
            ".min.js",
            ".bundle.js",
            ".generated.rs",
            ".designer.cs",
            ".g.dart",
            ".pb.go",
            ".gen.go",
        ]
        .into_iter()
        .map(str::to_string)
        .collect();

        Self {
            state_dir_name: ".uncle_funkle".to_string(),
            state_file_name: "state.json".to_string(),
            include_extensions,
            exclude_dirs,
            exclude_suffixes,
            max_file_bytes: 512 * 1024,
            max_concurrency: 16,
            detect_debug_artifacts: true,
            thresholds: DetectorThresholds::default(),
        }
    }
}

impl Default for DetectorThresholds {
    fn default() -> Self {
        Self {
            long_line: 120,
            large_file_lines: 400,
            long_function_lines: 80,
            deep_nesting_depth: 4,
            branch_points_per_function: 10,
            duplicate_window_lines: 6,
            duplicate_min_occurrences: 2,
            max_duplicate_reports: 16,
        }
    }
}

impl Config {
    pub fn state_dir(&self, root: &Path) -> PathBuf {
        root.join(&self.state_dir_name)
    }

    pub fn state_file(&self, root: &Path) -> PathBuf {
        self.state_dir(root).join(&self.state_file_name)
    }

    pub fn extension_in_scope(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| self.include_extensions.contains(&ext.to_ascii_lowercase()))
            .unwrap_or(false)
    }

    pub fn should_skip_dir_name(&self, dir_name: &str) -> bool {
        self.exclude_dirs.contains(dir_name)
    }

    pub fn should_skip_path(&self, path: &Path) -> bool {
        let rendered = path.to_string_lossy().replace('\\', "/");

        if self
            .exclude_suffixes
            .iter()
            .any(|suffix| rendered.ends_with(suffix))
        {
            return true;
        }

        path.components().any(|component| {
            component
                .as_os_str()
                .to_str()
                .map(|segment| self.exclude_dirs.contains(segment))
                .unwrap_or(false)
        })
    }
}
