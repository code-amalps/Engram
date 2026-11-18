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

/// Detect and return the appropriate framework provider for the given workspace.
/// Falls back to GenericPassThroughAdapter if no specific framework matches.
pub fn detect_framework_provider(
    workspace_root: &Path,
    files: &[String],
) -> Box<dyn FrameworkRelationshipProvider> {
    for provider in get_providers() {
        if provider.name() != "generic" && provider.detect(workspace_root, files) {
            return provider;
        }
    }
    Box::new(generic::GenericPassThroughAdapter::new())
}
