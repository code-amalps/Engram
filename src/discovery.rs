//! Production-grade configuration-driven directory discovery, ignore semantics,
//! and exclusion evaluation for Engram.
//!
//! Provides a unified, authoritative directory-discovery and ignore evaluation engine
//! shared across Analyzer, KnowledgeGraph, Incremental indexing, and Diagnostics.
//!
//! # Precedence Architecture:
//! 1. Universal Safety Exclusions (non-overridable: `.git`, `.engram`, `node_modules`, `target`, `.venv`, secrets, etc.)
//! 2. `engram.yaml` / `.engram/config.yaml` (`ignore.exclude` and `ignore.include` whitelists)
//! 3. `.engramignore` (Engram-specific ignores and `!` negations)
//! 4. `.gitignore` (Repository standard gitignore rules, enabled by default, configurable)
//!
//! Legitimate customer source directories (`reports/`, `datasets/`, `assets/`, `build/`, `results/`)
//! are fully discoverable by default unless explicitly excluded by one of the configuration sources.

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::{DirEntry, Walk, WalkBuilder};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Universal safety directory names that must be pruned at any depth
/// to prevent traversing dependency caches, compiler outputs, or engine metadata.
pub const UNIVERSAL_PRUNED_DIR_NAMES: &[&str] = &[
    ".git",
    ".engram",
    ".engram_cache",
    ".next",
    ".nuxt",
    ".turbo",
    ".venv",
    "venv",
    "env",
    "__pycache__",
    "node_modules",
    ".pytest_cache",
    ".mypy_cache",
    "graphify-out",
    "coverage",
    "test_out",
    "obj",
    "target",
    ".cargo",
    ".rustup",
];

/// Result of an ignore/exclusion decision with complete diagnostic explanation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExclusionDecision {
    pub relative_path: String,
    pub is_excluded: bool,
    pub reason: String,
    pub rule_source: Option<String>,
    pub matched_pattern: Option<String>,
}

/// Optional configuration schema for `engram.yaml` or `.engram/config.yaml`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct EngramConfig {
    #[serde(default)]
    pub ignore: Option<IgnoreConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IgnoreConfig {
    #[serde(default = "default_true")]
    pub use_gitignore: bool,
    #[serde(default = "default_true")]
    pub use_engramignore: bool,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub include: Vec<String>,
}

impl Default for IgnoreConfig {
    fn default() -> Self {
        Self {
            use_gitignore: true,
            use_engramignore: true,
            exclude: Vec::new(),
            include: Vec::new(),
        }
    }
}

fn default_true() -> bool {
    true
}

/// Normalizes a path on Windows by resolving canonical paths across existing ancestors
/// and stripping any leading verbatim `\\?\` prefix.
pub fn normalize_path(p: &Path) -> PathBuf {
    let mut ancestors = Vec::new();
    let mut curr = p;
    while !curr.exists() {
        if let Some(file_name) = curr.file_name() {
            ancestors.push(file_name);
        } else {
            break;
        }
        if let Some(parent) = curr.parent() {
            curr = parent;
        } else {
            break;
        }
    }
    let base_canon = if curr.exists() {
        fs::canonicalize(curr).unwrap_or_else(|_| curr.to_path_buf())
    } else {
        curr.to_path_buf()
    };
    let mut res = base_canon;
    for part in ancestors.into_iter().rev() {
        res.push(part);
    }
    let s = res.to_string_lossy();
    let trimmed = s.trim_start_matches(r"\\?\");
    PathBuf::from(trimmed)
}

/// Authoritative discovery and ignore evaluation engine.
pub struct DiscoveryEngine {
    root: PathBuf,
    yaml_matcher: Option<Gitignore>,
    engramignore_matcher: Option<Gitignore>,
    gitignore_matcher: Option<Gitignore>,
    pub use_gitignore: bool,
    pub use_engramignore: bool,
    legacy_patterns: Vec<(String, String)>,
}

impl DiscoveryEngine {
    /// Constructs a `DiscoveryEngine` for the given workspace root.
    pub fn for_root(root: &Path) -> Self {
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let root = normalize_path(&root);

        let mut use_gitignore = true;
        let mut use_engramignore = true;
        let mut yaml_matcher = None;

        // 1. Inspect engram.yaml or .engram/config.yaml
        let yaml_candidates = [
            root.join("engram.yaml"),
            root.join("engram.yml"),
            root.join(".engram").join("config.yaml"),
            root.join(".engram").join("config.yml"),
        ];

        for cfg_path in &yaml_candidates {
            if cfg_path.is_file() {
                if let Ok(content) = fs::read_to_string(cfg_path) {
                    if let Ok(cfg) = serde_yaml::from_str::<EngramConfig>(&content) {
                        if let Some(ig) = cfg.ignore {
                            use_gitignore = ig.use_gitignore;
                            use_engramignore = ig.use_engramignore;

                            let mut builder = GitignoreBuilder::new(&root);
                            for pat in &ig.exclude {
                                let _ = builder.add_line(Some(cfg_path.clone()), pat);
                            }
                            for pat in &ig.include {
                                let line = if pat.starts_with('!') {
                                    pat.clone()
                                } else {
                                    format!("!{pat}")
                                };
                                let _ = builder.add_line(Some(cfg_path.clone()), &line);
                            }
                            yaml_matcher = builder.build().ok();
                        }
                    }
                }
                break;
            }
        }

        // 2. Inspect .engramignore
        let mut legacy_patterns = Vec::new();
        let mut engramignore_matcher = None;
        if use_engramignore {
            let engramignore_path = root.join(".engramignore");
            if engramignore_path.is_file() {
                if let Ok(content) = fs::read_to_string(&engramignore_path) {
                    let mut builder = GitignoreBuilder::new(&root);
                    for line in content.lines().map(str::trim) {
                        if line.is_empty() || line.starts_with('#') {
                            continue;
                        }
                        if let Some((cat, pat)) = line.split_once(':') {
                            let cat_clean = cat.trim().to_ascii_uppercase();
                            let pat_clean = pat.trim().replace('\\', "/");
                            legacy_patterns.push((cat_clean.clone(), pat_clean.clone()));
                            if cat_clean != "SOURCE" && cat_clean != "GENERATED" {
                                let _ =
                                    builder.add_line(Some(engramignore_path.clone()), &pat_clean);
                            }
                        } else {
                            let pat_clean = line.replace('\\', "/");
                            let _ = builder.add_line(Some(engramignore_path.clone()), &pat_clean);
                        }
                    }
                    engramignore_matcher = builder.build().ok();
                }
            }
        }

        // 3. Inspect .gitignore
        let mut gitignore_matcher = None;
        if use_gitignore {
            let gitignore_path = root.join(".gitignore");
            if gitignore_path.is_file() {
                let mut builder = GitignoreBuilder::new(&root);
                let _ = builder.add(&gitignore_path);
                gitignore_matcher = builder.build().ok();
            }
        }

        Self {
            root,
            yaml_matcher,
            engramignore_matcher,
            gitignore_matcher,
            use_gitignore,
            use_engramignore,
            legacy_patterns,
        }
    }

    /// Constructs an `Arc<DiscoveryEngine>` for shared use across walk filters and indexing.
    pub fn new_shared(root: &Path) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self::for_root(root))
    }

    /// Constructs a `DiscoveryEngine` with additional explicit patterns (e.g. from legacy callers).
    pub fn for_root_with_patterns(root: &Path, extra_patterns: &[(String, String)]) -> Self {
        let mut engine = Self::for_root(root);
        for p in extra_patterns {
            if !engine.legacy_patterns.contains(p) {
                engine.legacy_patterns.push(p.clone());
            }
        }
        engine
    }

    /// Workspace root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Legacy patterns loaded from .engramignore (for backward compatibility).
    pub fn legacy_patterns(&self) -> &[(String, String)] {
        &self.legacy_patterns
    }

    /// Checks if a directory entry should be pruned during traversal.
    pub fn is_dir_entry_pruned(&self, entry: &DirEntry) -> bool {
        if !entry.file_type().is_some_and(|ft| ft.is_dir()) {
            return false;
        }

        let depth = entry.depth();
        if depth == 0 {
            return false;
        }

        let name = entry.file_name().to_string_lossy();

        // 1. Universal safety directories (any depth)
        if UNIVERSAL_PRUNED_DIR_NAMES.contains(&name.as_ref()) {
            return true;
        }

        // 2. Root-anchored Engram release folder
        if depth == 1 && name == "releases" {
            return true;
        }

        // 3. Engram benchmark hierarchy handling
        if depth == 2 {
            if let Some(parent) = entry.path().parent() {
                if parent.file_name().is_some_and(|p| p == "benchmarks") {
                    if name != "matched-pair-harness" {
                        return true;
                    }
                }
            }
        } else if depth == 3 {
            if let Some(parent) = entry.path().parent() {
                if parent
                    .file_name()
                    .is_some_and(|p| p == "matched-pair-harness")
                    && matches!(
                        name.as_ref(),
                        "runs"
                            | "reports"
                            | "workloads"
                            | "results"
                            | "datasets"
                            | "bin"
                            | "scratch"
                    )
                {
                    return true;
                }
            }
        }

        // 4. Configuration-based directory pruning (engram.yaml, .engramignore, .gitignore)
        let path_norm = normalize_path(entry.path());
        if let Ok(rel) = path_norm.strip_prefix(&self.root) {
            // Check engram.yaml
            if let Some(ref m) = self.yaml_matcher {
                let res = m.matched(rel, true);
                if res.is_ignore() {
                    return true;
                }
            }
            // Check .engramignore
            if let Some(ref m) = self.engramignore_matcher {
                let res = m.matched(rel, true);
                if res.is_ignore() {
                    return true;
                }
            }
            // Check .gitignore
            if let Some(ref m) = self.gitignore_matcher {
                let res = m.matched(rel, true);
                if res.is_ignore() {
                    return true;
                }
            }
        }

        false
    }

    /// Complete diagnostic explanation for why a path is included or excluded.
    pub fn explain(&self, path: &Path, is_dir: bool) -> ExclusionDecision {
        let path_norm = normalize_path(path);
        let Ok(rel) = path_norm.strip_prefix(&self.root) else {
            return ExclusionDecision {
                relative_path: path.to_string_lossy().to_string(),
                is_excluded: true,
                reason: "Path is outside the workspace root".into(),
                rule_source: Some("WorkspaceBoundary".into()),
                matched_pattern: None,
            };
        };

        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let parts: Vec<&str> = rel_str.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            return ExclusionDecision {
                relative_path: rel_str,
                is_excluded: false,
                reason: "Workspace root directory".into(),
                rule_source: None,
                matched_pattern: None,
            };
        }

        let check_dirs = if is_dir {
            &parts[..]
        } else if parts.len() > 1 {
            &parts[..parts.len() - 1]
        } else {
            &[][..]
        };

        // 1. Universal directory component exclusions (any depth)
        for part in check_dirs {
            if UNIVERSAL_PRUNED_DIR_NAMES.contains(part) {
                return ExclusionDecision {
                    relative_path: rel_str.clone(),
                    is_excluded: true,
                    reason: format!("Universal safety directory: '{part}'"),
                    rule_source: Some("UniversalSafety".into()),
                    matched_pattern: Some(part.to_string()),
                };
            }
        }

        // 2. Root-anchored Engram release archive exclusion
        if !check_dirs.is_empty() && check_dirs[0] == "releases" {
            return ExclusionDecision {
                relative_path: rel_str.clone(),
                is_excluded: true,
                reason: "Engram release package directory: 'releases'".into(),
                rule_source: Some("EngramArtifact".into()),
                matched_pattern: Some("releases/".into()),
            };
        }

        // 3. Benchmark hierarchy exclusions
        if !check_dirs.is_empty() && check_dirs[0] == "benchmarks" {
            if check_dirs.len() == 1 || check_dirs[1] != "matched-pair-harness" {
                return ExclusionDecision {
                    relative_path: rel_str.clone(),
                    is_excluded: true,
                    reason: "Benchmark sibling directory excluded from first-party indexing".into(),
                    rule_source: Some("BenchmarkAnchoring".into()),
                    matched_pattern: Some("benchmarks/*".into()),
                };
            }
            if check_dirs.len() >= 3
                && matches!(
                    check_dirs[2],
                    "runs" | "reports" | "workloads" | "results" | "datasets" | "bin" | "scratch"
                )
            {
                return ExclusionDecision {
                    relative_path: rel_str.clone(),
                    is_excluded: true,
                    reason: format!(
                        "Benchmark execution artifact directory: '{}'",
                        check_dirs[2]
                    ),
                    rule_source: Some("BenchmarkArtifact".into()),
                    matched_pattern: Some(format!(
                        "benchmarks/matched-pair-harness/{}/*",
                        check_dirs[2]
                    )),
                };
            }
        }

        // 4. Secret file exclusions
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
        if !is_dir {
            if (file_name == ".env" || file_name.starts_with(".env."))
                && file_name != ".env.example"
                && file_name != ".env.sample"
                && file_name != ".env.template"
            {
                return ExclusionDecision {
                    relative_path: rel_str.clone(),
                    is_excluded: true,
                    reason: format!("Environment secrets file: '{file_name}'"),
                    rule_source: Some("SecretFile".into()),
                    matched_pattern: Some(".env*".into()),
                };
            }
            if file_name.ends_with(".pem") || file_name.ends_with(".key") {
                return ExclusionDecision {
                    relative_path: rel_str.clone(),
                    is_excluded: true,
                    reason: format!("Cryptographic key file: '{file_name}'"),
                    rule_source: Some("SecretFile".into()),
                    matched_pattern: Some("*.pem/*.key".into()),
                };
            }
        }

        // 5. Tiered Configuration Evaluation (Layer 2: engram.yaml -> Layer 3: .engramignore -> Layer 4: .gitignore)
        // Layer 2: Root Configuration (engram.yaml / .engram/config.yaml)
        if let Some(ref m) = self.yaml_matcher {
            let res = m.matched(rel, is_dir);
            if res.is_whitelist() {
                return ExclusionDecision {
                    relative_path: rel_str.clone(),
                    is_excluded: false,
                    reason: "Explicitly whitelisted in engram.yaml".into(),
                    rule_source: Some("engram.yaml".into()),
                    matched_pattern: None,
                };
            }
            for i in 1..=check_dirs.len() {
                let dir_subpath = check_dirs[..i].join("/");
                let res = m.matched(&dir_subpath, true);
                if res.is_ignore() {
                    return ExclusionDecision {
                        relative_path: rel_str.clone(),
                        is_excluded: true,
                        reason: format!("Parent directory '{dir_subpath}' excluded by engram.yaml"),
                        rule_source: Some("engram.yaml".into()),
                        matched_pattern: None,
                    };
                }
            }
            if res.is_ignore() {
                return ExclusionDecision {
                    relative_path: rel_str.clone(),
                    is_excluded: true,
                    reason: "Excluded by engram.yaml configuration".into(),
                    rule_source: Some("engram.yaml".into()),
                    matched_pattern: None,
                };
            }
        }

        // Layer 3: Explicit tool ignore (.engramignore)
        if let Some(ref m) = self.engramignore_matcher {
            let res = m.matched(rel, is_dir);
            if res.is_whitelist() {
                return ExclusionDecision {
                    relative_path: rel_str.clone(),
                    is_excluded: false,
                    reason: "Explicitly whitelisted in .engramignore".into(),
                    rule_source: Some(".engramignore".into()),
                    matched_pattern: None,
                };
            }
            for i in 1..=check_dirs.len() {
                let dir_subpath = check_dirs[..i].join("/");
                let res = m.matched(&dir_subpath, true);
                if res.is_ignore() {
                    return ExclusionDecision {
                        relative_path: rel_str.clone(),
                        is_excluded: true,
                        reason: format!(
                            "Parent directory '{dir_subpath}' excluded by .engramignore"
                        ),
                        rule_source: Some(".engramignore".into()),
                        matched_pattern: None,
                    };
                }
            }
            if res.is_ignore() {
                return ExclusionDecision {
                    relative_path: rel_str.clone(),
                    is_excluded: true,
                    reason: "Excluded by .engramignore pattern".into(),
                    rule_source: Some(".engramignore".into()),
                    matched_pattern: None,
                };
            }
        }

        // Layer 4: Standard VCS ignore (.gitignore)
        if let Some(ref m) = self.gitignore_matcher {
            let res = m.matched(rel, is_dir);
            if res.is_whitelist() {
                return ExclusionDecision {
                    relative_path: rel_str.clone(),
                    is_excluded: false,
                    reason: "Explicitly whitelisted in .gitignore".into(),
                    rule_source: Some(".gitignore".into()),
                    matched_pattern: None,
                };
            }
            for i in 1..=check_dirs.len() {
                let dir_subpath = check_dirs[..i].join("/");
                let res = m.matched(&dir_subpath, true);
                if res.is_ignore() {
                    return ExclusionDecision {
                        relative_path: rel_str.clone(),
                        is_excluded: true,
                        reason: format!("Parent directory '{dir_subpath}' excluded by .gitignore"),
                        rule_source: Some(".gitignore".into()),
                        matched_pattern: None,
                    };
                }
            }
            if res.is_ignore() {
                return ExclusionDecision {
                    relative_path: rel_str.clone(),
                    is_excluded: true,
                    reason: "Excluded by .gitignore pattern".into(),
                    rule_source: Some(".gitignore".into()),
                    matched_pattern: None,
                };
            }
        }

        // 6. Check legacy category-prefixed patterns from .engramignore
        for (cat, pat) in &self.legacy_patterns {
            if cat != "SOURCE" && cat != "GENERATED" && cat != "SECRET" {
                if crate::analyzer::matches_pattern(&rel_str, pat) {
                    return ExclusionDecision {
                        relative_path: rel_str.clone(),
                        is_excluded: true,
                        reason: format!("Excluded by legacy pattern [{cat}]: {pat}"),
                        rule_source: Some(".engramignore".into()),
                        matched_pattern: Some(pat.clone()),
                    };
                }
            } else if cat == "SECRET" && crate::analyzer::matches_pattern(&rel_str, pat) {
                return ExclusionDecision {
                    relative_path: rel_str.clone(),
                    is_excluded: true,
                    reason: format!("Excluded by legacy secret pattern: {pat}"),
                    rule_source: Some(".engramignore".into()),
                    matched_pattern: Some(pat.clone()),
                };
            }
        }

        // If not excluded by any rule, it is eligible for discovery
        ExclusionDecision {
            relative_path: rel_str,
            is_excluded: false,
            reason: "Eligible for source indexing".into(),
            rule_source: None,
            matched_pattern: None,
        }
    }

    /// Fast boolean check for whether a file or directory is excluded.
    pub fn is_excluded(&self, path: &Path, is_dir: bool) -> bool {
        self.explain(path, is_dir).is_excluded
    }

    /// Builds a pre-configured `WalkBuilder` that applies the engine's directory pruning.
    pub fn walk_builder(&self) -> WalkBuilder {
        let mut builder = WalkBuilder::new(&self.root);
        builder
            .hidden(false)
            .git_ignore(false) // We manage gitignore directly in the engine
            .git_global(false)
            .git_exclude(false)
            .follow_links(false);
        builder
    }

    /// Constructs a pre-configured `Walk` iterator that prunes directories according
    /// to universal safety rules and configured ignore patterns.
    pub fn build_walk(self: &std::sync::Arc<Self>) -> Walk {
        let this = self.clone();
        let mut builder = self.walk_builder();
        builder.filter_entry(move |entry| !this.is_dir_entry_pruned(entry));
        builder.build()
    }
}
