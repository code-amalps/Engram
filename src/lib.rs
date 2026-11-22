//! Engram: Local engineering memory and change intelligence for AI coding agents.

pub mod analyzer;
pub mod archaeology;
pub mod bench;
pub mod budget;
pub mod ci_guard;
pub mod compiler;
pub mod constitution;
pub mod contract;
pub mod doctor;
pub mod drift;
pub mod empirical;
pub mod empirical_live;
pub mod empirical_runner;
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
    pub const RUSTC_VERSION: &str = env!("ENGRAM_BUILD_RUSTC");
    pub const BUILD_PROFILE: &str = env!("ENGRAM_BUILD_PROFILE");
    pub const VERSION_FULL: &str = concat!(
        env!("CARGO_PKG_VERSION"),
        "\ncommit: ",
        env!("ENGRAM_BUILD_GIT_COMMIT"),
        "\ndirty: ",
        env!("ENGRAM_BUILD_GIT_DIRTY"),
        "\nrustc: ",
        env!("ENGRAM_BUILD_RUSTC"),
        "\nprofile: ",
        env!("ENGRAM_BUILD_PROFILE")
    );
}

