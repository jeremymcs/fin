// Fin — Extension Registry
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use super::api::{Extension, ExtensionContext, ExtensionManifest, ExtensionTier};
use crate::tools::AgentTool;

/// Manages loaded extensions.
pub struct ExtensionRegistry {
    extensions: Vec<Box<dyn Extension>>,
    disabled: Vec<String>,
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
            disabled: Vec::new(),
        }
    }

    /// Create registry with all built-in extensions.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();

        // Register built-in extensions
        registry.register(Box::new(super::builtin::web_search::WebSearchExtension));
        registry.register(Box::new(super::builtin::context7::Context7Extension));

        registry
    }

    pub fn register(&mut self, ext: Box<dyn Extension>) {
        let manifest = ext.manifest();
        if !self.disabled.contains(&manifest.id) {
            self.extensions.push(ext);
        }
    }

    #[allow(dead_code)]
    pub fn disable(&mut self, id: &str) -> bool {
        // Can't disable core extensions
        if let Some(ext) = self.extensions.iter().find(|e| e.manifest().id == id) {
            if ext.manifest().tier == ExtensionTier::Core {
                return false;
            }
        }
        self.disabled.push(id.to_string());
        self.extensions.retain(|e| e.manifest().id != id);
        true
    }

    /// Get all tools from all extensions.
    pub fn tools(&self) -> Vec<Box<dyn AgentTool>> {
        self.extensions.iter().flat_map(|e| e.tools()).collect()
    }

    /// Get prompt additions from all extensions.
    pub fn prompt_additions(&self, ctx: &ExtensionContext) -> String {
        self.extensions
            .iter()
            .filter_map(|e| e.prompt_additions(ctx))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Notify all extensions of session start.
    pub fn on_session_start(&self, ctx: &ExtensionContext) {
        for ext in &self.extensions {
            ext.on_session_start(ctx);
        }
    }

    #[allow(dead_code)]
    /// List loaded extensions.
    pub fn list(&self) -> Vec<ExtensionManifest> {
        self.extensions.iter().map(|e| e.manifest()).collect()
    }
}
