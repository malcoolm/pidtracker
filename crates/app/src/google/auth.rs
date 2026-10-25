//! Přihlášení Googlem (Google Identity Services, token v prohlížeči).

use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::config::GOOGLE_CLIENT_ID;
use crate::interop::{gis_request_token, gis_revoke};
use crate::util::{keys, ls_get, ls_remove, ls_set, ls_set_str};

pub const SCOPE: &str = "https://www.googleapis.com/auth/spreadsheets openid email";
/// Token považujeme za neplatný o minutu dřív, než vyprší.
const EXPIRY_MARGIN_MS: f64 = 60_000.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Token {
    pub access_token: String,
    pub expires_at_ms: f64,
    pub email: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AuthError {
    NoClientId,
    Failed(String),
}

impl AuthError {
    pub fn message(&self) -> String {
        match self {
            AuthError::NoClientId => "Chybí Google Client ID v buildu (PIDTRACKER_GOOGLE_CLIENT_ID).".into(),
            AuthError::Failed(m) => format!("Přihlášení se nezdařilo: {m}"),
        }
    }
}

pub fn current_token() -> Option<Token> {
    ls_get::<Token>(keys::TOKEN).filter(|t| t.expires_at_ms - EXPIRY_MARGIN_MS > js_sys::Date::now())
}

/// Spustí přihlášení. Musí se volat přímo z obsluhy kliknutí: popup se otevře synchronně
/// a teprve vrácená future čeká na výsledek.
pub fn sign_in(login_hint: Option<&str>) -> impl Future<Output = Result<Token, AuthError>> + 'static {
    let promise = GOOGLE_CLIENT_ID.map(|id| gis_request_token(id, SCOPE, login_hint.unwrap_or("")));
    async move {
        let promise = promise.ok_or(AuthError::NoClientId)?;
        let started = js_sys::Date::now();
        let resp = JsFuture::from(promise).await.map_err(|e| AuthError::Failed(js_text(&e)))?;
        let get = |k: &str| js_sys::Reflect::get(&resp, &k.into()).ok();
        let access_token = get("access_token")
            .and_then(|v| v.as_string())
            .ok_or_else(|| AuthError::Failed("Google nevrátil token".into()))?;
        let expires_in = get("expires_in")
            .and_then(|v| v.as_f64().or_else(|| v.as_string().and_then(|s| s.parse().ok())))
            .unwrap_or(3599.0);
        let email = fetch_email(&access_token).await?;
        let token = Token { access_token, expires_at_ms: started + expires_in * 1000.0, email: email.clone() };
        ls_set(keys::TOKEN, &token);
        ls_set_str(keys::ME, &email);
        Ok(token)
    }
}

async fn fetch_email(access_token: &str) -> Result<String, AuthError> {
    #[derive(Deserialize)]
    struct UserInfo {
        email: String,
    }
    let resp = gloo_net::http::Request::get("https://openidconnect.googleapis.com/v1/userinfo")
        .header("Authorization", &format!("Bearer {access_token}"))
        .send()
        .await
        .map_err(|e| AuthError::Failed(e.to_string()))?;
    if !resp.ok() {
        return Err(AuthError::Failed(format!("userinfo HTTP {}", resp.status())));
    }
    let info: UserInfo = resp.json().await.map_err(|e| AuthError::Failed(e.to_string()))?;
    Ok(info.email.to_lowercase())
}

/// Zapomene token (např. po 401). E-mail zůstává, aby šlo dál zaznamenávat jízdy.
pub fn drop_token() {
    ls_remove(keys::TOKEN);
}

pub fn sign_out() {
    if let Some(t) = ls_get::<Token>(keys::TOKEN) {
        gis_revoke(&t.access_token);
    }
    ls_remove(keys::TOKEN);
    ls_remove(keys::ME);
}

fn js_text(v: &JsValue) -> String {
    v.as_string().unwrap_or_else(|| format!("{v:?}"))
}
