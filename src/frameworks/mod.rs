//! Framework Relationship Provider SPI (Service Provider Interface).
//!
//! Provides extensible, language- and framework-specific semantic discovery
//! decoupled from the generic query router.

pub mod angular;
pub mod generic;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AdapterConfidence {
    Candidate,
    Pattern,
    Exact,
}

impl AdapterConfidence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Candidate => "CANDIDATE",
            Self::Pattern => "PATTERN",
            Self::Exact => "EXACT",
        }
    }

    pub fn weight(&self) -> f64 {
        match self {
            Self::Candidate => 0.6,
            Self::Pattern => 0.85,
            Self::Exact => 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CompanionKind {
    StateAction,
    StateEffect,
    StateReducer,
    StateSelector,
    ComponentTemplate,
    ComponentStyle,
    TranslationDictionary,
    InjectedService,
    ModuleDeclaration,
    StoreRegistration,
    RelatedTest,
    RouteDefinition,
    InterfaceModel,
}

impl CompanionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::StateAction => "STATE_ACTION",
            Self::StateEffect => "STATE_EFFECT",
            Self::StateReducer => "STATE_REDUCER",
            Self::StateSelector => "STATE_SELECTOR",
            Self::ComponentTemplate => "COMPONENT_TEMPLATE",
            Self::ComponentStyle => "COMPONENT_STYLE",
            Self::TranslationDictionary => "TRANSLATION_DICTIONARY",
            Self::InjectedService => "INJECTED_SERVICE",
            Self::ModuleDeclaration => "MODULE_DECLARATION",
            Self::StoreRegistration => "STORE_REGISTRATION",
            Self::RelatedTest => "RELATED_TEST",
            Self::RouteDefinition => "ROUTE_DEFINITION",
            Self::InterfaceModel => "INTERFACE_MODEL",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArchitecturalCompanion {
    pub path: String,
    pub kind: CompanionKind,
    pub confidence: f64,
    pub reason: String,
    pub exists_on_disk: bool,
}

/// SPI for framework-specific architectural relationship discovery.
pub trait FrameworkRelationshipProvider: Send + Sync {
    /// Identifier of the framework adapter (e.g. "angular_ngrx", "generic")
    fn name(&self) -> &'static str;

    /// Detect whether the current workspace uses this framework
    fn detect(&self, workspace_root: &Path, files: &[String]) -> bool;

    /// Detect framework presence with explicit confidence level
    fn detect_confidence(
        &self,
        workspace_root: &Path,
        files: &[String],
    ) -> Option<AdapterConfidence> {
        if self.detect(workspace_root, files) {
            Some(AdapterConfidence::Pattern)
        } else {
            None
        }
    }

    /// Given a seed entity or file, discover related architectural companion files
    fn expand_companions(
        &self,
        workspace_root: &Path,
        conn: &Connection,
        seed_path: &str,
    ) -> Vec<ArchitecturalCompanion>;

    /// Given a task description, discover domain-specific companion seeds
    fn task_companion_seeds(
        &self,
        workspace_root: &Path,
        conn: &Connection,
        task: &str,
    ) -> Vec<ArchitecturalCompanion>;
}

/// Get all registered framework relationship providers.
pub fn get_providers() -> Vec<Box<dyn FrameworkRelationshipProvider>> {
    vec![
        Box::new(angular::AngularNgRxAdapter::new()),
        Box::new(generic::GenericPassThroughAdapter::new()),
    ]
}

/// Detect all applicable framework adapters with their confidence levels.
pub fn detect_all_framework_adapters(
    workspace_root: &Path,
    files: &[String],
) -> Vec<(Box<dyn FrameworkRelationshipProvider>, AdapterConfidence)> {
    let mut matches = Vec::new();
    for provider in get_providers() {
        if provider.name() == "generic" {
            continue;
        }
        if let Some(conf) = provider.detect_confidence(workspace_root, files) {
            matches.push((provider, conf));
        }
    }
    if matches.is_empty() {
        matches.push((
            Box::new(generic::GenericPassThroughAdapter::new()),
            AdapterConfidence::Candidate,
        ));
    }
    matches
}

/// Composite multi-framework relationship provider.
pub struct MultiFrameworkProvider {
    adapters: Vec<(Box<dyn FrameworkRelationshipProvider>, AdapterConfidence)>,
}

impl MultiFrameworkProvider {
    pub fn new(adapters: Vec<(Box<dyn FrameworkRelationshipProvider>, AdapterConfidence)>) -> Self {
        Self { adapters }
    }
}

impl FrameworkRelationshipProvider for MultiFrameworkProvider {
    fn name(&self) -> &'static str {
        "multi_framework"
    }

    fn detect(&self, _workspace_root: &Path, _files: &[String]) -> bool {
        !self.adapters.is_empty()
    }

    fn detect_confidence(
        &self,
        _workspace_root: &Path,
        _files: &[String],
    ) -> Option<AdapterConfidence> {
        self.adapters.iter().map(|(_, c)| *c).max()
    }

    fn expand_companions(
        &self,
        workspace_root: &Path,
        conn: &Connection,
        seed_path: &str,
    ) -> Vec<ArchitecturalCompanion> {
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for (adapter, conf) in &self.adapters {
            let companions = adapter.expand_companions(workspace_root, conn, seed_path);
            for mut comp in companions {
                comp.confidence *= conf.weight();
                if seen.insert((comp.path.clone(), comp.kind)) {
                    results.push(comp);
                }
            }
        }
        results
    }

    fn task_companion_seeds(
        &self,
        workspace_root: &Path,
        conn: &Connection,
        task: &str,
    ) -> Vec<ArchitecturalCompanion> {
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for (adapter, conf) in &self.adapters {
            let companions = adapter.task_companion_seeds(workspace_root, conn, task);
            for mut comp in companions {
                comp.confidence *= conf.weight();
                if seen.insert((comp.path.clone(), comp.kind)) {
                    results.push(comp);
                }
            }
        }
        results
    }
}

/// Detect and return the appropriate framework provider for the given workspace.
/// If multiple frameworks match, returns a MultiFrameworkProvider aggregating them.
/// Falls back to GenericPassThroughAdapter if no specific framework matches.
pub fn detect_framework_provider(
    workspace_root: &Path,
    files: &[String],
) -> Box<dyn FrameworkRelationshipProvider> {
    let detected = detect_all_framework_adapters(workspace_root, files);
    if detected.len() == 1 {
        detected.into_iter().next().unwrap().0
    } else {
        Box::new(MultiFrameworkProvider::new(detected))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_confidence_ordering_and_weights() {
        assert!(AdapterConfidence::Exact > AdapterConfidence::Pattern);
        assert!(AdapterConfidence::Pattern > AdapterConfidence::Candidate);
        assert!((AdapterConfidence::Exact.weight() - 1.0).abs() < f64::EPSILON);
        assert!(AdapterConfidence::Pattern.weight() < 1.0);
        assert!(AdapterConfidence::Candidate.weight() < AdapterConfidence::Pattern.weight());
    }

    #[test]
    fn test_generic_fallback_detection() {
        let temp = std::env::temp_dir().join(format!("engram_gen_test_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&temp);

        let files = vec!["src/main.rs".to_string(), "Cargo.toml".to_string()];
        let detected = detect_all_framework_adapters(&temp, &files);
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].0.name(), "generic");
        assert_eq!(detected[0].1, AdapterConfidence::Candidate);

        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_angular_pattern_detection() {
        let temp = std::env::temp_dir().join(format!("engram_ang_test_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&temp);

        let files = vec![
            "src/app/hero.component.ts".to_string(),
            "src/app/hero.actions.ts".to_string(),
        ];
        let detected = detect_all_framework_adapters(&temp, &files);
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].0.name(), "angular_ngrx");
        assert_eq!(detected[0].1, AdapterConfidence::Pattern);

        let _ = std::fs::remove_dir_all(&temp);
    }
}
