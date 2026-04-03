// Fin + Extension API Trait

use crate::tools::AgentTool;

/// Context provided to extensions during lifecycle events.
#[allow(dead_code)] // Fields read by extension implementations
pub struct ExtensionContext {
    pub cwd: std::path::PathBuf,
    pub session_id: String,
}

/// Manifest describing an extension's capabilities.
#[allow(dead_code)] // Fields used by extension registry and display
#[derive(Debug, Clone)]
pub struct ExtensionManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub tier: ExtensionTier,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExtensionTier {
    Core,    // Cannot be disabled
    Bundled, // Enabled by default, can be disabled
    #[allow(dead_code)] // Future: user-installed extensions
    Community, // User-installed
}

/// Trait all extensions implement.
pub trait Extension: Send + Sync {
    /// Extension manifest.
    fn manifest(&self) -> ExtensionManifest;

    /// Register tools provided by this extension.
    fn tools(&self) -> Vec<Box<dyn AgentTool>> {
        Vec::new()
    }

    /// System prompt additions (appended to base prompt).
    fn prompt_additions(&self, _ctx: &ExtensionContext) -> Option<String> {
        None
    }

    /// Called when a session starts.
    fn on_session_start(&self, _ctx: &ExtensionContext) {}

    /// Called when a session ends.
    #[allow(dead_code)] // Lifecycle hook for extension implementations
    fn on_session_end(&self, _ctx: &ExtensionContext) {}

    /// Called before each agent turn.
    #[allow(dead_code)] // Lifecycle hook for extension implementations
    fn on_before_turn(&self, _ctx: &ExtensionContext) {}

    /// Called after each agent turn.
    #[allow(dead_code)] // Lifecycle hook for extension implementations
    fn on_after_turn(&self, _ctx: &ExtensionContext) {}
}
