// Fin + OAuth Helpers

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::auth::AuthStore;

pub const GOOGLE_OAUTH_SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/cloud-platform",
    "https://www.googleapis.com/auth/generative-language.retriever",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleOAuthCredentials {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub client_id: String,
    pub client_secret: String,
    pub token_uri: String,
    pub project_id: Option<String>,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GoogleOAuthFlowResult {
    pub credentials: GoogleOAuthCredentials,
    pub client_secret_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct GoogleDesktopClientConfig {
    pub client_id: String,
    pub client_secret: String,
    pub auth_uri: String,
    pub token_uri: String,
    pub project_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleClientSecretFile {
    installed: GoogleInstalledClient,
}

#[derive(Debug, Deserialize)]
struct GoogleInstalledClient {
    client_id: String,
    client_secret: String,
    auth_uri: String,
    token_uri: String,
    project_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
    #[serde(default)]
    scope: Option<String>,
}

pub fn run_google_oauth_flow(client_secret_path: &Path) -> anyhow::Result<GoogleOAuthFlowResult> {
    let client_secret_path = expand_tilde(client_secret_path);
    if !client_secret_path.exists() {
        anyhow::bail!(
            "Google OAuth client file not found: {}",
            client_secret_path.display()
        );
    }

    let client = load_google_desktop_client(&client_secret_path)?;
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    listener.set_nonblocking(true)?;
    let redirect_uri = format!("http://127.0.0.1:{}", listener.local_addr()?.port());
    let state = uuid::Uuid::new_v4().to_string();
    let scopes = GOOGLE_OAUTH_SCOPES.join(" ");

    let auth_url = reqwest::Url::parse_with_params(
        &client.auth_uri,
        &[
            ("client_id", client.client_id.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("response_type", "code"),
            ("scope", scopes.as_str()),
            ("access_type", "offline"),
            ("prompt", "consent"),
            ("state", state.as_str()),
        ],
    )?;

    open_url_in_browser(auth_url.as_str())?;

    println!("Opened Google OAuth in your browser.");
    println!("Waiting for the callback on {} ...", redirect_uri);

    let callback = wait_for_google_callback(&listener, &state)?;
    let credentials = exchange_google_auth_code(client, &callback.code, &redirect_uri)?;
    if credentials.refresh_token.is_none() {
        anyhow::bail!(
            "Google OAuth completed without a refresh token. Re-run /login google and approve offline access."
        );
    }

    Ok(GoogleOAuthFlowResult {
        credentials,
        client_secret_path,
    })
}

pub async fn ensure_google_access_token(
    client: &reqwest::Client,
    auth_file: &Path,
    creds: &mut GoogleOAuthCredentials,
) -> anyhow::Result<String> {
    let now = Utc::now().timestamp();
    if let Some(expires_at) = creds.expires_at {
        if expires_at > now + 60 {
            return Ok(creds.access_token.clone());
        }
    } else if !creds.access_token.is_empty() {
        return Ok(creds.access_token.clone());
    }

    let refresh_token = creds
        .refresh_token
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Google OAuth refresh token missing. Run /login google again."))?;

    let response = client
        .post(&creds.token_uri)
        .form(&[
            ("client_id", creds.client_id.as_str()),
            ("client_secret", creds.client_secret.as_str()),
            ("refresh_token", refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Google OAuth refresh failed ({status}): {body}");
    }

    let token: GoogleTokenResponse = response.json().await?;
    creds.access_token = token.access_token;
    creds.expires_at = token.expires_in.map(|secs| Utc::now().timestamp() + secs);
    if let Some(scope) = token.scope {
        creds.scopes = scope.split_whitespace().map(ToString::to_string).collect();
    }

    let mut auth = AuthStore::load(auth_file).unwrap_or_default();
    auth.set_google_oauth(creds.clone());
    auth.save(auth_file)?;

    Ok(creds.access_token.clone())
}

pub fn load_google_desktop_client(path: &Path) -> anyhow::Result<GoogleDesktopClientConfig> {
    let content = std::fs::read_to_string(path)?;
    let file: GoogleClientSecretFile = serde_json::from_str(&content)?;

    Ok(GoogleDesktopClientConfig {
        client_id: file.installed.client_id,
        client_secret: file.installed.client_secret,
        auth_uri: file.installed.auth_uri,
        token_uri: file.installed.token_uri,
        project_id: file.installed.project_id,
    })
}

fn exchange_google_auth_code(
    client: GoogleDesktopClientConfig,
    code: &str,
    redirect_uri: &str,
) -> anyhow::Result<GoogleOAuthCredentials> {
    let redirect_uri = redirect_uri.to_string();
    let code = code.to_string();

    std::thread::spawn(move || -> anyhow::Result<GoogleOAuthCredentials> {
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(async move {
            let http = reqwest::Client::new();
            let response = http
                .post(&client.token_uri)
                .form(&[
                    ("client_id", client.client_id.as_str()),
                    ("client_secret", client.client_secret.as_str()),
                    ("code", code.as_str()),
                    ("grant_type", "authorization_code"),
                    ("redirect_uri", redirect_uri.as_str()),
                ])
                .send()
                .await?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                anyhow::bail!("Google OAuth token exchange failed ({status}): {body}");
            }

            let token: GoogleTokenResponse = response.json().await?;
            Ok(GoogleOAuthCredentials {
                access_token: token.access_token,
                refresh_token: token.refresh_token,
                expires_at: token.expires_in.map(|secs| Utc::now().timestamp() + secs),
                client_id: client.client_id,
                client_secret: client.client_secret,
                token_uri: client.token_uri,
                project_id: client.project_id,
                scopes: token
                    .scope
                    .map(|scope| scope.split_whitespace().map(ToString::to_string).collect())
                    .unwrap_or_else(|| {
                        GOOGLE_OAUTH_SCOPES
                            .iter()
                            .map(|scope| (*scope).to_string())
                            .collect()
                    }),
            })
        })
    })
    .join()
    .map_err(|_| anyhow::anyhow!("Google OAuth exchange thread panicked"))?
}

struct GoogleCallback {
    code: String,
}

fn wait_for_google_callback(listener: &TcpListener, expected_state: &str) -> anyhow::Result<GoogleCallback> {
    let deadline = Instant::now() + Duration::from_secs(180);

    loop {
        match listener.accept() {
            Ok((mut stream, _addr)) => {
                let mut request_line = String::new();
                {
                    let mut reader = BufReader::new(stream.try_clone()?);
                    reader.read_line(&mut request_line)?;
                }

                let target = request_line
                    .split_whitespace()
                    .nth(1)
                    .ok_or_else(|| anyhow::anyhow!("invalid Google OAuth callback request"))?;
                let callback_url = reqwest::Url::parse(&format!("http://localhost{target}"))?;
                let query: std::collections::HashMap<String, String> =
                    callback_url.query_pairs().into_owned().collect();

                let body = if let Some(error) = query.get("error") {
                    format!("Authentication failed: {error}")
                } else {
                    "Authentication complete. You can return to fin.".to_string()
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes())?;
                stream.flush()?;

                if let Some(error) = query.get("error") {
                    anyhow::bail!("Google OAuth failed: {error}");
                }

                let state = query
                    .get("state")
                    .ok_or_else(|| anyhow::anyhow!("missing OAuth state in callback"))?;
                if state != expected_state {
                    anyhow::bail!("Google OAuth state mismatch");
                }

                let code = query
                    .get("code")
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("missing OAuth code in callback"))?;
                return Ok(GoogleCallback { code });
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    anyhow::bail!("Timed out waiting for Google OAuth callback");
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(err) => return Err(err.into()),
        }
    }
}

fn open_url_in_browser(url: &str) -> anyhow::Result<()> {
    let status = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).status()?
    } else if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .status()?
    } else {
        std::process::Command::new("xdg-open").arg(url).status()?
    };

    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("browser opener exited with status: {status}");
    }
}

fn expand_tilde(path: &Path) -> PathBuf {
    let path_str = path.to_string_lossy();
    if let Some(rest) = path_str.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    path.to_path_buf()
}
