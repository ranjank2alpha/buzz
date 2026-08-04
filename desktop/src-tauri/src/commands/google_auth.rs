use axum::{
    extract::{Query, State as AxumState},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
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
const GOOGLE_COMPANY_SALT: &[u8] = b"buzz_k2alpha_company_salt_v1";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleAuthResult {
    pub identity: IdentityInfo,
    pub email: String,
    pub name: Option<String>,
    pub google_sub: String,
    pub nsec: String,
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
}

struct CallbackState {
    sender: Mutex<Option<oneshot::Sender<Result<String, String>>>>,
}

async fn oauth_callback(
    Query(query): Query<HashMap<String, String>>,
    AxumState(state): AxumState<std::sync::Arc<CallbackState>>,
) -> Response {
    let result = match query.get("code").filter(|code| !code.is_empty()) {
        Some(code) => Ok(code.clone()),
        None => Err(query
            .get("error_description")
            .or_else(|| query.get("error"))
            .cloned()
            .unwrap_or_else(|| "Google authentication was cancelled or failed.".to_owned())),
    };

    if let Some(sender) = state
        .sender
        .lock()
        .expect("callback sender poisoned")
        .take()
    {
        let _ = sender.send(result);
    }

    Html(
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
</html>"#,
    )
    .into_response()
}

fn derive_nostr_secret_key(google_sub: &str) -> Result<SecretKey, String> {
    let mut hasher = Sha256::new();
    hasher.update(GOOGLE_COMPANY_SALT);
    hasher.update(google_sub.as_bytes());
    let hash = hasher.finalize();

    SecretKey::from_slice(&hash).map_err(|e| format!("invalid derived secret key: {e}"))
}

#[tauri::command]
pub async fn start_google_workspace_login(
    app_handle: tauri::AppHandle,
) -> Result<GoogleAuthResult, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("failed to bind local callback port: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("failed to read local port: {e}"))?
        .port();

    let redirect_uri = format!("http://127.0.0.1:{port}/auth/callback");

    let (sender, receiver) = oneshot::channel();
    let callback_state = std::sync::Arc::new(CallbackState {
        sender: Mutex::new(Some(sender)),
    });

    let router = Router::new()
        .route("/auth/callback", get(oauth_callback))
        .with_state(callback_state);

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    let mut auth_url = url::Url::parse("https://accounts.google.com/o/oauth2/v2/auth")
        .map_err(|e| e.to_string())?;
    auth_url
        .query_pairs_mut()
        .append_pair("client_id", GOOGLE_CLIENT_ID)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", "openid email profile")
        .append_pair("hd", ALLOWED_DOMAIN)
        .append_pair("prompt", "select_account");

    tauri_plugin_opener::OpenerExt::opener(&app_handle)
        .open_url(auth_url.as_str(), None::<&str>)
        .map_err(|e| format!("failed to open browser: {e}"))?;

    let code_result = tokio::time::timeout(LOGIN_TIMEOUT, receiver)
        .await
        .map_err(|_| "Authentication timed out. Please try signing in again.".to_string())?
        .map_err(|_| "Callback channel closed.".to_string())?;

    let _ = shutdown_tx.send(());
    let code = code_result?;

    let client = reqwest::Client::new();
    let token_resp: TokenResponse = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", code.as_str()),
            ("client_id", GOOGLE_CLIENT_ID),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect_uri.as_str()),
        ])
        .send()
        .await
        .map_err(|e| format!("failed token exchange request: {e}"))?
        .json()
        .await
        .map_err(|e| format!("failed parsing token response: {e}"))?;

    let id_token_str = token_resp
        .id_token
        .ok_or_else(|| "Google token response did not contain id_token".to_string())?;

    let claims = parse_id_token_claims(&id_token_str)?;

    let user_email = claims.email.unwrap_or_default().trim().to_lowercase();
    let user_domain = claims.hd.unwrap_or_default().trim().to_lowercase();

    if !user_email.ends_with(&format!("@{ALLOWED_DOMAIN}")) && user_domain != ALLOWED_DOMAIN {
        return Err(format!(
            "Access Restricted: '{user_email}' is not a @{ALLOWED_DOMAIN} account."
        ));
    }

    let state = app_handle.state::<AppState>();
    let existing_nsec = {
        let keys = state.keys.lock().ok();
        keys.and_then(|k| k.secret_key().to_bech32().ok())
    };

    let nsec = match existing_nsec {
        Some(nsec) => nsec,
        None => {
            let derived_sk = derive_nostr_secret_key(&claims.sub)?;
            derived_sk
                .to_bech32()
                .map_err(|e| format!("failed encoding derived nsec: {e}"))?
        }
    };

    let identity = crate::commands::identity::import_identity(nsec.clone(), None, app_handle).await?;

    Ok(GoogleAuthResult {
        identity,
        email: user_email,
        name: claims.name,
        google_sub: claims.sub,
        nsec,
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
    fn test_derive_nostr_secret_key_is_deterministic() {
        let sub = "108392019482019382";
        let key1 = derive_nostr_secret_key(sub).unwrap();
        let key2 = derive_nostr_secret_key(sub).unwrap();

        assert_eq!(key1.to_bech32().unwrap(), key2.to_bech32().unwrap());
    }

    #[test]
    fn test_derive_nostr_secret_key_differs_for_different_subs() {
        let key1 = derive_nostr_secret_key("sub_alice").unwrap();
        let key2 = derive_nostr_secret_key("sub_bob").unwrap();

        assert_ne!(key1.to_bech32().unwrap(), key2.to_bech32().unwrap());
    }

    #[test]
    fn test_parse_id_token_claims_valid_payload() {
        let payload = r#"{"sub":"12345","email":"alice@k2alpha.ai","hd":"k2alpha.ai","name":"Alice"}"#;
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
    }
}
