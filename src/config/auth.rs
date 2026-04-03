// Fin + Auth Storage (API Keys — Keyring + File Fallback)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

const KEYRING_SERVICE: &str = "fin-cli";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStore {
    pub providers: HashMap<String, ProviderAuth>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderAuth {
    ApiKey { key: String },
    BearerToken { token: String },
    GoogleOAuth {
        access_token: String,
        refresh_token: Option<String>,
        expires_at: Option<i64>,
        client_id: String,
        client_secret: String,
        token_uri: String,
        project_id: Option<String>,
        scopes: Vec<String>,
    },
}

impl Default for AuthStore {
    /// Default loads from the auth file on disk if it exists.
    /// This is the primary entry point — callsites use AuthStore::default().
    fn default() -> Self {
        match crate::config::paths::FinPaths::resolve() {
            Ok(paths) => Self::load(&paths.auth_file).unwrap_or_else(|e| {
                tracing::warn!("Failed to load auth.json: {e}");
                Self::empty()
            }),
            Err(_) => Self::empty(),
        }
    }
}

impl AuthStore {
    /// Create an empty store (no providers loaded).
    pub fn empty() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Load auth from a specific file path.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::empty());
        }
        let content = std::fs::read_to_string(path)?;
        let store: Self = serde_json::from_str(&content)?;
        Ok(store)
    }

    /// Save auth to file (with restrictive permissions).
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, &content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }

        Ok(())
    }

    /// Get API key for a provider, checking: env vars → keyring → stored auth file.
    pub fn get_api_key(&self, provider: &str) -> Option<String> {
        if provider == "openai" {
            // OAuth bearer tokens (Codex/OpenAI session style) are accepted first.
            // Falls back to standard API key auth.
            return std::env::var("OPENAI_ACCESS_TOKEN")
                .ok()
                .or_else(|| std::env::var("OPENAI_BEARER_TOKEN").ok())
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                .or_else(|| self.get_keyring_key(provider))
                .or_else(|| self.get_stored_key(provider));
        }

        let env_key = match provider {
            "anthropic" => "ANTHROPIC_API_KEY",
            "google" => "GOOGLE_API_KEY",
            "mistral" => "MISTRAL_API_KEY",
            _ => {
                return self
                    .get_keyring_key(provider)
                    .or_else(|| self.get_stored_key(provider));
            }
        };

        std::env::var(env_key)
            .ok()
            .or_else(|| self.get_keyring_key(provider))
            .or_else(|| self.get_stored_key(provider))
    }

    fn get_stored_key(&self, provider: &str) -> Option<String> {
        match self.providers.get(provider)? {
            ProviderAuth::ApiKey { key } => Some(key.clone()),
            ProviderAuth::BearerToken { token } => Some(token.clone()),
            ProviderAuth::GoogleOAuth { access_token, .. } => Some(access_token.clone()),
        }
    }

    /// Try to read an API key from the OS keyring.
    fn get_keyring_key(&self, provider: &str) -> Option<String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, provider).ok()?;
        entry.get_password().ok()
    }

    /// Set an API key for a provider.
    /// Stores in keyring (preferred) and in-memory map. Falls back to file-only.
    pub fn set_api_key(&mut self, provider: &str, key: String) {
        // Try keyring first
        if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, provider) {
            if entry.set_password(&key).is_ok() {
                tracing::debug!("Stored {provider} key in OS keyring");
            } else {
                tracing::debug!("Keyring unavailable for {provider}, using file storage");
            }
        }
        // Always store in the file-backed map as fallback
        self.providers
            .insert(provider.to_string(), ProviderAuth::ApiKey { key });
    }

    /// Store a bearer token for a provider.
    pub fn set_bearer_token(&mut self, provider: &str, token: String) {
        if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, provider) {
            if entry.set_password(&token).is_ok() {
                tracing::debug!("Stored {provider} bearer token in OS keyring");
            } else {
                tracing::debug!("Keyring unavailable for {provider}, using file storage");
            }
        }
        self.providers
            .insert(provider.to_string(), ProviderAuth::BearerToken { token });
    }

    /// Store Google OAuth credentials.
    pub fn set_google_oauth(&mut self, creds: crate::config::oauth::GoogleOAuthCredentials) {
        self.providers.insert(
            "google".to_string(),
            ProviderAuth::GoogleOAuth {
                access_token: creds.access_token,
                refresh_token: creds.refresh_token,
                expires_at: creds.expires_at,
                client_id: creds.client_id,
                client_secret: creds.client_secret,
                token_uri: creds.token_uri,
                project_id: creds.project_id,
                scopes: creds.scopes,
            },
        );
    }

    pub fn get_google_oauth(&self) -> Option<crate::config::oauth::GoogleOAuthCredentials> {
        match self.providers.get("google")? {
            ProviderAuth::GoogleOAuth {
                access_token,
                refresh_token,
                expires_at,
                client_id,
                client_secret,
                token_uri,
                project_id,
                scopes,
            } => Some(crate::config::oauth::GoogleOAuthCredentials {
                access_token: access_token.clone(),
                refresh_token: refresh_token.clone(),
                expires_at: *expires_at,
                client_id: client_id.clone(),
                client_secret: client_secret.clone(),
                token_uri: token_uri.clone(),
                project_id: project_id.clone(),
                scopes: scopes.clone(),
            }),
            _ => None,
        }
    }

    /// Remove an API key from both keyring and stored file.
    pub fn remove_api_key(&mut self, provider: &str) {
        // Remove from keyring
        if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, provider) {
            let _ = entry.delete_credential();
        }
        // Remove from in-memory map
        self.providers.remove(provider);
    }

    /// Get a masked version of a key (last 4 chars visible).
    pub fn get_masked_key(&self, provider: &str) -> Option<String> {
        self.get_api_key(provider).map(|key| {
            if key.len() <= 4 {
                "****".to_string()
            } else {
                let visible = &key[key.len() - 4..];
                format!("{}…{}", &key[..3], visible)
            }
        })
    }

    /// Check if any provider is configured (stored keys, keyring, env vars, or local Ollama).
    pub fn has_any_provider(&self) -> bool {
        // Check stored keys first
        if !self.providers.is_empty() {
            return true;
        }
        // Then env vars
        if std::env::var("ANTHROPIC_API_KEY").is_ok()
            || std::env::var("OPENAI_API_KEY").is_ok()
            || std::env::var("OPENAI_ACCESS_TOKEN").is_ok()
            || std::env::var("OPENAI_BEARER_TOKEN").is_ok()
            || std::env::var("GOOGLE_API_KEY").is_ok()
            || std::env::var("GOOGLE_CLOUD_PROJECT").is_ok()
            || std::env::var("CLOUDSDK_CORE_PROJECT").is_ok()
            || std::env::var("AWS_ACCESS_KEY_ID").is_ok()
            || std::env::var("AWS_PROFILE").is_ok()
        {
            return true;
        }
        // Check if Ollama is reachable (quick sync check)
        Self::check_ollama_available()
    }

    /// Quick synchronous check if Ollama is reachable.
    fn check_ollama_available() -> bool {
        let host =
            std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string());
        // Extract host:port from URL
        let addr_str = host
            .strip_prefix("http://")
            .or_else(|| host.strip_prefix("https://"))
            .unwrap_or(&host);
        if let Ok(addr) = addr_str.parse::<std::net::SocketAddr>() {
            std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(500))
                .is_ok()
        } else {
            // Try with default port if no port specified
            let with_port = if addr_str.contains(':') {
                addr_str.to_string()
            } else {
                format!("{addr_str}:11434")
            };
            with_port
                .parse::<std::net::SocketAddr>()
                .map(|addr| {
                    std::net::TcpStream::connect_timeout(
                        &addr,
                        std::time::Duration::from_millis(500),
                    )
                    .is_ok()
                })
                .unwrap_or(false)
        }
    }
}
