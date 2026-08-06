use axum::{
    extract::{Query, State as AxumState},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use nostr::{SecretKey, ToBech32};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Mutex, time::Duration};
use tauri::Manager;
use tokio::{net::TcpListener, sync::oneshot};

use crate::{app_state::AppState, models::IdentityInfo};

const GOOGLE_CLIENT_ID: &str =
    "928375928891-mjfo59obr65fldcehesvbq0cve94ease.apps.googleusercontent.com";
const ALLOWED_DOMAIN: &str = "k2alpha.ai";
const LOGIN_TIMEOUT: Duration = Duration::from_secs(5 * 60);

fn is_secret_configured(secret: Option<&str>) -> bool {
    secret.map(|s| !s.trim().is_empty()).unwrap_or(false)
}

#[tauri::command]
pub fn google_sso_available() -> bool {
    is_secret_configured(option_env!("BUZZ_DESKTOP_BUILD_GOOGLE_CLIENT_SECRET"))
        && is_secret_configured(option_env!("BUZZ_DESKTOP_BUILD_IDENTITY_PEPPER"))
}

fn derive_nostr_secret_key(pepper: &str, google_sub: &str) -> Result<SecretKey, String> {
    let mut hasher = Sha256::new();
    hasher.update(pepper.as_bytes());
    hasher.update(google_sub.as_bytes());
    let hash: [u8; 32] = hasher.finalize().into();
    SecretKey::from_slice(&hash)
        .map_err(|e| format!("failed creating secret key from derived hash: {e}"))
}

fn check_safety_guards(keyring_locked: bool, identity_lost: bool) -> Result<(), String> {
    if keyring_locked {
        return Err("Your secure storage is currently locked. Please unlock your keyring and retry.".into());
    }
    if identity_lost {
        return Err("Identity is lost. Please restore from backup and try again.".into());
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleAuthResult {
    pub identity: IdentityInfo,
    pub email: String,
    pub name: Option<String>,
    pub is_fresh_key: bool,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    id_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IdTokenClaims {
    sub: String,
    email: Option<String>,
    hd: Option<String>,
    name: Option<String>,
    aud: String,
    exp: u64,
    email_verified: Option<bool>,
    iss: Option<String>,
}

struct CallbackState {
    sender: Mutex<Option<oneshot::Sender<Result<String, String>>>>,
    state: String,
}

struct ServerShutdownGuard {
    tx: Option<oneshot::Sender<()>>,
}

impl Drop for ServerShutdownGuard {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(());
        }
    }
}

async fn oauth_callback(
    Query(query): Query<HashMap<String, String>>,
    AxumState(state): AxumState<std::sync::Arc<CallbackState>>,
) -> Response {
    let result = if query.get("state") != Some(&state.state) {
        tracing::warn!("OAuth callback rejected: state mismatch or missing");
        Err("Invalid state parameter. Authentication aborted.".to_owned())
    } else {
        match query.get("code").filter(|code| !code.is_empty()) {
            Some(code) => Ok(code.clone()),
            None => Err(query
                .get("error_description")
                .or_else(|| query.get("error"))
                .cloned()
                .unwrap_or_else(|| "Google authentication was cancelled or failed.".to_owned())),
        }
    };

    if let Err(ref err) = result {
        tracing::error!("OAuth callback failure: {err}");
    }

    if let Ok(mut lock) = state.sender.lock() {
        if let Some(sender) = lock.take() {
            let _ = sender.send(result.clone());
        } else {
            tracing::warn!("OAuth callback received but sender already used");
        }
    } else {
        tracing::error!("OAuth callback sender mutex poisoned");
    }

    let html = render_oauth_callback_html(result.is_ok());

    Html(html).into_response()
}

fn render_oauth_callback_html(is_success: bool) -> &'static str {
    if is_success {
        r#"<!doctype html>
<html>
<head><title>Authentication Complete</title></head>
<body style="font-family: ui-sans-serif, system-ui, sans-serif; display: grid; place-items: center; min-height: 100vh; background: #0f172a; color: #f8fafc; margin: 0;">
<div style="text-align: center; padding: 2.5rem; border-radius: 16px; background: #1e293b; border: 1px solid #334155; max-width: 400px;">
  <div style="font-size: 48px; margin-bottom: 12px;">🐝</div>
  <h2 style="margin: 0 0 8px 0; font-size: 20px;">Authenticated with @k2alpha.ai</h2>
  <p style="margin: 0; color: #94a3b8; font-size: 14px;">You can now close this tab and return to the Buzz desktop app.</p>
</div>
</body>
</html>"#
    } else {
        r#"<!doctype html>
<html>
<head><title>Authentication Failed</title></head>
<body style="font-family: ui-sans-serif, system-ui, sans-serif; display: grid; place-items: center; min-height: 100vh; background: #0f172a; color: #f8fafc; margin: 0;">
<div style="text-align: center; padding: 2.5rem; border-radius: 16px; background: #1e293b; border: 1px solid #334155; max-width: 400px;">
  <div style="font-size: 48px; margin-bottom: 12px;">⚠️</div>
  <h2 style="margin: 0 0 8px 0; font-size: 20px;">Authentication Failed</h2>
  <p style="margin: 0; color: #94a3b8; font-size: 14px;">Google authentication failed. Please return to the Buzz desktop app and try again.</p>
</div>
</body>
</html>"#
    }
}

#[tauri::command]
pub async fn start_google_workspace_login(
    app_handle: tauri::AppHandle,
) -> Result<GoogleAuthResult, String> {
    tracing::info!("Starting Google Workspace SSO login flow");

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| {
            tracing::error!("Failed to bind local callback port: {e}");
            format!("failed to bind local callback port: {e}")
        })?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("failed to read local port: {e}"))?
        .port();

    let redirect_uri = format!("http://127.0.0.1:{port}/auth/callback");

    let oauth_state = uuid::Uuid::new_v4().simple().to_string();

    let (sender, receiver) = oneshot::channel();
    let callback_state = std::sync::Arc::new(CallbackState {
        sender: Mutex::new(Some(sender)),
        state: oauth_state.clone(),
    });

    let router = Router::new()
        .route("/auth/callback", get(oauth_callback))
        .with_state(callback_state);

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let _guard = ServerShutdownGuard {
        tx: Some(shutdown_tx),
    };

    tokio::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    let code_verifier = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
    
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let code_challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());

    let mut auth_url = url::Url::parse("https://accounts.google.com/o/oauth2/v2/auth")
        .map_err(|e| e.to_string())?;
    auth_url
        .query_pairs_mut()
        .append_pair("client_id", GOOGLE_CLIENT_ID)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", "openid email profile")
        .append_pair("hd", ALLOWED_DOMAIN)
        .append_pair("prompt", "select_account")
        .append_pair("state", &oauth_state)
        .append_pair("code_challenge", &code_challenge)
        .append_pair("code_challenge_method", "S256");

    tauri_plugin_opener::OpenerExt::opener(&app_handle)
        .open_url(auth_url.as_str(), None::<&str>)
        .map_err(|e| format!("failed to open browser: {e}"))?;

    let code_result = tokio::time::timeout(LOGIN_TIMEOUT, receiver)
        .await
        .map_err(|_| {
            tracing::warn!("OAuth authentication timed out");
            "Authentication timed out. Please try signing in again.".to_string()
        })?
        .map_err(|e| {
            tracing::error!("OAuth callback channel closed: {e}");
            "Callback channel closed.".to_string()
        })?;

    // Drop guard will handle the server shutdown automatically.
    let code = code_result?;

    let client_secret = option_env!("BUZZ_DESKTOP_BUILD_GOOGLE_CLIENT_SECRET")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Google SSO is not available in this build (missing client secret).".to_string())?;

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;
    let token_res = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", code.as_str()),
            ("client_id", GOOGLE_CLIENT_ID),
            ("client_secret", client_secret),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect_uri.as_str()),
            ("code_verifier", code_verifier.as_str()),
        ])
        .send()
        .await
        .map_err(|e| format!("failed token exchange request: {e}"))?;

    if !token_res.status().is_success() {
        let status = token_res.status();
        let body = token_res.text().await.unwrap_or_default();
        tracing::error!("Google token exchange failed ({status}): {body}");
        return Err(format!("Google token exchange failed ({status}). Please try again."));
    }

    let token_resp: TokenResponse = token_res
        .json()
        .await
        .map_err(|e| format!("failed parsing token response: {e}"))?;

    let id_token_str = token_resp
        .id_token
        .ok_or_else(|| "Google token response did not contain id_token".to_string())?;

    let claims = parse_id_token_claims(&id_token_str)?;

    let valid_issuers = ["https://accounts.google.com", "accounts.google.com"];
    let iss = claims.iss.as_deref().unwrap_or_default();
    if !valid_issuers.contains(&iss) {
        tracing::error!("Token issuer mismatch or missing: {iss}");
        return Err("Invalid token issuer.".to_string());
    }

    if claims.aud != GOOGLE_CLIENT_ID {
        tracing::error!("Token audience mismatch: {}", claims.aud);
        return Err("Invalid token audience.".to_string());
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    if claims.exp < now {
        tracing::error!("Token has expired");
        return Err("Authentication token expired. Please try again.".to_string());
    }

    if claims.email_verified != Some(true) {
        tracing::error!("Google account email is not verified");
        return Err("Email address is not verified by Google.".to_string());
    }

    let user_email = claims.email.unwrap_or_default().trim().to_lowercase();
    let user_domain = claims.hd.unwrap_or_default().trim().to_lowercase();

    if !user_email.ends_with(&format!("@{ALLOWED_DOMAIN}")) && user_domain != ALLOWED_DOMAIN {
        return Err(format!(
            "Access Restricted: '{user_email}' is not a @{ALLOWED_DOMAIN} account."
        ));
    }

    let state = app_handle.state::<AppState>();
    let keyring_locked = state.keyring_locked.load(std::sync::atomic::Ordering::Acquire);
    let identity_lost = state.identity_lost.load(std::sync::atomic::Ordering::Acquire);
    check_safety_guards(keyring_locked, identity_lost)?;

    let pepper = option_env!("BUZZ_DESKTOP_BUILD_IDENTITY_PEPPER")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Google SSO is not available in this build (missing identity pepper).".to_string())?;

    let secret_key = derive_nostr_secret_key(pepper, &claims.sub)?;
    let keys = nostr::Keys::new(secret_key);
    let nsec = keys
        .secret_key()
        .to_bech32()
        .map_err(|e| format!("failed encoding derived nsec: {e}"))?;
    let derived_pubkey = keys.public_key().to_hex();

    let existing_pubkey = {
        let keys = state.keys.lock().ok();
        keys.map(|k| k.public_key().to_hex())
    };
    let has_persisted_identity = state.identity_storage() != crate::app_state::IdentityStorage::Ephemeral;

    if let Some(existing_pubkey) = existing_pubkey {
        if existing_pubkey != derived_pubkey {
            tracing::warn!(
                "Replacing existing identity pubkey {} with derived Google SSO pubkey {}",
                existing_pubkey,
                derived_pubkey
            );
        }
    }

    let is_fresh_key = !has_persisted_identity;

    let identity = crate::commands::identity::import_identity(nsec, None, app_handle).await?;

    tracing::info!("Google SSO flow completed successfully for {}", user_email);

    Ok(GoogleAuthResult {
        identity,
        email: user_email,
        name: claims.name,
        is_fresh_key,
    })
}

fn parse_id_token_claims(id_token: &str) -> Result<IdTokenClaims, String> {
    let parts: Vec<&str> = id_token.split('.').collect();
    if parts.len() < 2 {
        return Err("Invalid ID token format".to_string());
    }

    let payload_b64 = parts[1];
    let decoded = base64::Engine::decode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        payload_b64,
    )
    .or_else(|_| {
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, payload_b64)
    })
    .map_err(|e| format!("failed decoding JWT payload: {e}"))?;

    serde_json::from_slice::<IdTokenClaims>(&decoded)
        .map_err(|e| format!("failed parsing JWT claims: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_configured_check() {
        assert!(!is_secret_configured(None));
        assert!(!is_secret_configured(Some("")));
        assert!(!is_secret_configured(Some("   ")));
        assert!(is_secret_configured(Some("secret-value")));
    }

    #[test]
    fn test_derive_nostr_secret_key() {
        let pepper1 = "pepper-1";
        let pepper2 = "pepper-2";
        let sub1 = "sub-12345";
        let sub2 = "sub-67890";

        // Same pepper + same sub -> identical key across repeated calls
        let key1_a = derive_nostr_secret_key(pepper1, sub1).unwrap();
        let key1_b = derive_nostr_secret_key(pepper1, sub1).unwrap();
        assert_eq!(key1_a, key1_b);

        // Same sub + different peppers -> different keys
        let key_diff_pepper = derive_nostr_secret_key(pepper2, sub1).unwrap();
        assert_ne!(key1_a, key_diff_pepper);

        // Different subs + same pepper -> different keys
        let key_diff_sub = derive_nostr_secret_key(pepper1, sub2).unwrap();
        assert_ne!(key1_a, key_diff_sub);
    }

    #[test]
    fn test_oauth_callback_html_rendering() {
        let success_html = render_oauth_callback_html(true);
        let failure_html = render_oauth_callback_html(false);

        assert_ne!(success_html, failure_html);
        assert!(success_html.contains("Authenticated with @k2alpha.ai"));
        assert!(failure_html.contains("Authentication Failed"));
        assert!(!failure_html.contains("Authenticated with @k2alpha.ai"));
    }

    #[test]
    fn test_parse_id_token_claims_valid_payload() {
        let payload = r#"{"sub":"12345","email":"alice@k2alpha.ai","hd":"k2alpha.ai","name":"Alice","aud":"test_client_id","exp":9999999999,"email_verified":true,"iss":"https://accounts.google.com"}"#;
        let b64_payload = base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            payload,
        );
        let jwt = format!("header.{b64_payload}.signature");

        let claims = parse_id_token_claims(&jwt).unwrap();
        assert_eq!(claims.sub, "12345");
        assert_eq!(claims.email.as_deref(), Some("alice@k2alpha.ai"));
        assert_eq!(claims.hd.as_deref(), Some("k2alpha.ai"));
        assert_eq!(claims.name.as_deref(), Some("Alice"));
        assert_eq!(claims.aud, "test_client_id");
        assert_eq!(claims.exp, 9999999999);
        assert_eq!(claims.email_verified, Some(true));
        assert_eq!(claims.iss.as_deref(), Some("https://accounts.google.com"));
    }

    #[test]
    fn test_parse_id_token_claims_email_verified_and_iss() {
        // Missing email_verified or false
        let payload_unverified = r#"{"sub":"12345","email":"alice@k2alpha.ai","hd":"k2alpha.ai","aud":"test_client_id","exp":9999999999,"email_verified":false,"iss":"https://accounts.google.com"}"#;
        let b64_unverified = base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            payload_unverified,
        );
        let jwt_unverified = format!("header.{b64_unverified}.sig");
        let claims_unverified = parse_id_token_claims(&jwt_unverified).unwrap();
        assert_eq!(claims_unverified.email_verified, Some(false));

        let payload_no_verified = r#"{"sub":"12345","email":"alice@k2alpha.ai","hd":"k2alpha.ai","aud":"test_client_id","exp":9999999999,"iss":"accounts.google.com"}"#;
        let b64_no_verified = base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            payload_no_verified,
        );
        let jwt_no_verified = format!("header.{b64_no_verified}.sig");
        let claims_no_verified = parse_id_token_claims(&jwt_no_verified).unwrap();
        assert_eq!(claims_no_verified.email_verified, None);
        assert_eq!(claims_no_verified.iss.as_deref(), Some("accounts.google.com"));
    }

    #[test]
    fn test_safety_guards_matrix() {
        // Unlocked and not lost -> Ok
        assert!(check_safety_guards(false, false).is_ok());

        // Keyring locked -> Err
        assert!(check_safety_guards(true, false).is_err());

        // Identity lost -> Err
        assert!(check_safety_guards(false, true).is_err());
    }
}
