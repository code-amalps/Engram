//! Engram: Local engineering memory and change intelligence for AI coding agents.

pub mod analyzer;
pub mod archaeology;
pub mod bench;
pub mod budget;
pub mod ci_guard;
pub mod compiler;
pub mod constitution;
pub mod contract;
pub mod discovery;
pub mod doctor;
pub mod drift;
pub mod empirical;
pub mod empirical_live;
pub mod empirical_runner;
pub mod engage;
pub mod feature_trace;
pub mod flywheel;
pub mod frameworks;
pub mod gc;
pub mod git;
pub mod git_history;
pub mod go_parser;
pub mod graph;
pub mod guard;
pub mod hook;
pub mod impact;
pub mod incremental;
pub mod instructions;
pub mod java_parser;
pub mod knowledge_graph;
pub mod manifest;
pub mod mcp;
pub mod memory;
pub mod memory_graph;
pub mod model;
pub mod printer;
pub mod prompt;
pub mod recovery;
pub mod setup;
pub mod storage;
pub mod telemetry;

pub mod build_info {
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
    pub const GIT_COMMIT: &str = env!("ENGRAM_BUILD_GIT_COMMIT");
    pub const GIT_DIRTY: &str = env!("ENGRAM_BUILD_GIT_DIRTY");
    pub const PROVENANCE_SOURCE: &str = env!("ENGRAM_BUILD_PROVENANCE_SOURCE");
    pub const SOURCE_FINGERPRINT: &str = env!("ENGRAM_BUILD_SOURCE_FINGERPRINT");
    pub const LOCKFILE_SHA256: &str = env!("ENGRAM_BUILD_LOCKFILE_SHA256");
    pub const BUILD_ID: &str = env!("ENGRAM_BUILD_ID");
    pub const RUSTC_VERSION: &str = env!("ENGRAM_BUILD_RUSTC");
    pub const BUILD_PROFILE: &str = env!("ENGRAM_BUILD_PROFILE");
    pub const TARGET_TRIPLE: &str = env!("ENGRAM_BUILD_TARGET");
    pub const SCHEMA_VERSION: &str = env!("ENGRAM_BUILD_SCHEMA_VERSION");
    pub const HOST_TRIPLE: &str = env!("ENGRAM_BUILD_HOST");
    pub const OPT_LEVEL: &str = env!("ENGRAM_BUILD_OPT_LEVEL");
    pub const DEBUG_ASSERTIONS: &str = env!("ENGRAM_BUILD_DEBUG_ASSERTIONS");
    pub const CARGO_FEATURES: &str = env!("ENGRAM_BUILD_FEATURES");
    pub const RUSTFLAGS: &str = env!("ENGRAM_BUILD_RUSTFLAGS");
    pub const DEBUG_LEVEL: &str = env!("ENGRAM_BUILD_DEBUG_LEVEL");
    pub const PANIC_STRATEGY: &str = env!("ENGRAM_BUILD_PANIC_STRATEGY");
    pub const COMPILATION_DIRTY: &str = env!("ENGRAM_BUILD_COMPILATION_DIRTY");
    pub const WORKING_TREE_DIRTY: &str = env!("ENGRAM_BUILD_WORKING_TREE_DIRTY");
    pub const RUSTC_COMMIT: &str = env!("ENGRAM_BUILD_RUSTC_COMMIT");
    pub const RUSTC_LLVM: &str = env!("ENGRAM_BUILD_RUSTC_LLVM");
    pub const LINKER_NAME: &str = env!("ENGRAM_BUILD_LINKER_NAME");
    pub const LINKER_SHA256: &str = env!("ENGRAM_BUILD_LINKER_SHA256");
    pub const LINKER_FLAGS: &str = env!("ENGRAM_BUILD_LINKER_FLAGS");

    pub const VERSION_FULL: &str = concat!(
        env!("CARGO_PKG_VERSION"),
        "\ncommit: ",
        env!("ENGRAM_BUILD_GIT_COMMIT"),
        "\ndirty: ",
        env!("ENGRAM_BUILD_GIT_DIRTY"),
        "\nrustc: ",
        env!("ENGRAM_BUILD_RUSTC"),
        "\nprofile: ",
        env!("ENGRAM_BUILD_PROFILE"),
        "\nprovenance: ",
        env!("ENGRAM_BUILD_PROVENANCE_SOURCE"),
        "\nfingerprint: ",
        env!("ENGRAM_BUILD_SOURCE_FINGERPRINT"),
        "\nbuild_id: ",
        env!("ENGRAM_BUILD_ID")
    );

    /// Produces a deterministic, machine-readable JSON string matching the build identity schema.
    pub fn build_info_json() -> String {
        serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "package_version": VERSION,
            "source_identity": {
                "git_commit": if GIT_COMMIT == "unknown" || GIT_COMMIT == "invalid" { serde_json::Value::Null } else { serde_json::json!(GIT_COMMIT) },
                "git_dirty": GIT_DIRTY,
                "compilation_dirty": COMPILATION_DIRTY,
                "working_tree_dirty": WORKING_TREE_DIRTY,
                "source_fingerprint": if SOURCE_FINGERPRINT == "unknown" || SOURCE_FINGERPRINT == "invalid" { serde_json::Value::Null } else { serde_json::json!(SOURCE_FINGERPRINT) },
                "provenance_source": PROVENANCE_SOURCE,
                "lockfile_sha256": if LOCKFILE_SHA256 == "unknown" || LOCKFILE_SHA256 == "invalid" { serde_json::Value::Null } else { serde_json::json!(LOCKFILE_SHA256) }
            },
            "build_environment": {
                "build_profile": BUILD_PROFILE,
                "rustc_version": RUSTC_VERSION,
                "rustc_commit": RUSTC_COMMIT,
                "rustc_llvm": RUSTC_LLVM,
                "linker_name": LINKER_NAME,
                "linker_sha256": LINKER_SHA256,
                "linker_flags": LINKER_FLAGS,
                "target_triple": TARGET_TRIPLE,
                "host_triple": HOST_TRIPLE,
                "opt_level": OPT_LEVEL,
                "debug_assertions": DEBUG_ASSERTIONS,
                "debug_level": DEBUG_LEVEL,
                "panic_strategy": PANIC_STRATEGY,
                "cargo_features": CARGO_FEATURES,
                "rustflags": RUSTFLAGS
            },
            "build_id": BUILD_ID
        }).to_string()
    }
}
