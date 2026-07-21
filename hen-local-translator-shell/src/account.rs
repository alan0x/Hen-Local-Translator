use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use rand::{rngs::OsRng, RngCore};
use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, process::Command};
use url::Url;

use crate::preferences;

const KEYCHAIN_SERVICE: &str = "com.henlocal.translator";
const SESSION_ACCOUNT: &str = "supabase-session-v1";
const DEVICE_KEY_ACCOUNT: &str = "device-signing-key-v1";
const PKCE_ACCOUNT: &str = "pending-pkce-v1";
const LICENSE_CLOCK_ACCOUNT: &str = "license-clock-watermark-v1";
const REDIRECT_URL: &str = "henlocal://auth/callback";
const LICENSE_KEY_ID: &str = "hen-local-license-v1";
const LICENSE_PUBLIC_KEY: &str = include_str!("../license-public-key.b64");

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileConfig {
    supabase_url: Option<String>,
    supabase_anon_key: Option<String>,
    auth_provider: Option<String>,
    account_url: Option<String>,
}

#[derive(Debug, Clone)]
struct AccountConfig {
    supabase_url: String,
    anon_key: String,
    auth_provider: String,
    account_url: Option<String>,
}

impl AccountConfig {
    fn load() -> Self {
        let file = fs::read_to_string(preferences::preferences_dir().join("account_config.json"))
            .ok()
            .and_then(|content| serde_json::from_str::<FileConfig>(&content).ok())
            .unwrap_or_default();
        let value = |runtime: &str, compiled: Option<&str>, from_file: Option<String>| {
            std::env::var(runtime)
                .ok()
                .or_else(|| compiled.map(str::to_string))
                .or(from_file)
                .unwrap_or_default()
                .trim_end_matches('/')
                .to_string()
        };
        Self {
            supabase_url: value(
                "HEN_LOCAL_SUPABASE_URL",
                option_env!("HEN_LOCAL_SUPABASE_URL"),
                file.supabase_url,
            ),
            anon_key: value(
                "HEN_LOCAL_SUPABASE_ANON_KEY",
                option_env!("HEN_LOCAL_SUPABASE_ANON_KEY"),
                file.supabase_anon_key,
            ),
            auth_provider: value(
                "HEN_LOCAL_AUTH_PROVIDER",
                option_env!("HEN_LOCAL_AUTH_PROVIDER"),
                file.auth_provider,
            )
            .if_empty("google"),
            account_url: {
                let result = value(
                    "HEN_LOCAL_ACCOUNT_URL",
                    option_env!("HEN_LOCAL_ACCOUNT_URL"),
                    file.account_url,
                );
                (!result.is_empty()).then_some(result)
            },
        }
    }

    fn configured(&self) -> bool {
        !self.supabase_url.is_empty() && !self.anon_key.is_empty()
    }
}

trait EmptyFallback {
    fn if_empty(self, fallback: &str) -> String;
}

impl EmptyFallback for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredSession {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    email: String,
}

#[derive(Debug, Deserialize)]
struct TokenUser {
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
    user: Option<TokenUser>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LicensePayload {
    version: u32,
    key_id: String,
    user_id: String,
    device_id: String,
    device_public_key: String,
    issued_at: String,
    expires_at: String,
    plan: String,
    unlimited_local_translation: bool,
    source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LicenseEnvelope {
    payload: LicensePayload,
    canonical_payload: String,
    signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDevice {
    pub id: String,
    pub friendly_name: String,
    pub activated_at: String,
    pub last_used_at: String,
    pub deactivated_at: Option<String>,
    pub current: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStatus {
    pub configured: bool,
    pub signed_in: bool,
    pub email: Option<String>,
    pub subscription_state: String,
    pub entitlement_source: String,
    pub access_until: Option<String>,
    pub license_valid: bool,
    pub lease_expires_at: Option<String>,
    pub current_device_id: Option<String>,
    pub devices: Vec<AccountDevice>,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PkceAttempt {
    verifier: String,
    state: String,
}

pub struct AccountManager {
    config: AccountConfig,
    http: Client,
    pending_pkce: parking_lot::Mutex<Option<PkceAttempt>>,
    cached_status: parking_lot::Mutex<AccountStatus>,
}

impl AccountManager {
    pub fn new() -> Self {
        let config = AccountConfig::load();
        let status = AccountStatus {
            configured: config.configured(),
            signed_in: false,
            email: None,
            subscription_state: "not_configured".into(),
            entitlement_source: "not_configured".into(),
            access_until: None,
            license_valid: false,
            lease_expires_at: None,
            current_device_id: None,
            devices: Vec::new(),
            message: if config.configured() {
                "Sign in to start or refresh your license".into()
            } else {
                "Account service is not configured in this internal build".into()
            },
        };
        let manager = Self {
            config,
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("account HTTP client should build"),
            pending_pkce: parking_lot::Mutex::new(None),
            cached_status: parking_lot::Mutex::new(status),
        };
        manager.update_local_status();
        manager
    }

    pub fn status(&self) -> AccountStatus {
        self.update_local_status();
        self.cached_status.lock().clone()
    }

    pub fn begin_sign_in(&self) -> Result<AccountStatus, String> {
        self.require_config()?;
        let verifier = random_urlsafe(48);
        let state = random_urlsafe(24);
        let callback_url = format!("{REDIRECT_URL}?state={state}");
        let challenge =
            general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let url = if let Some(account_url) = &self.config.account_url {
            let mut url =
                Url::parse(account_url).map_err(|error| format!("Invalid account URL: {error}"))?;
            url.path_segments_mut()
                .map_err(|_| "Account URL cannot be a base URL".to_string())?
                .push("desktop-sign-in");
            url.query_pairs_mut()
                .append_pair("redirect_to", &callback_url)
                .append_pair("code_challenge", &challenge)
                .append_pair("code_challenge_method", "s256")
                .append_pair("state", &state);
            url
        } else {
            let mut url = Url::parse(&format!("{}/auth/v1/authorize", self.config.supabase_url))
                .map_err(|error| format!("Invalid Supabase URL: {error}"))?;
            url.query_pairs_mut()
                .append_pair("provider", &self.config.auth_provider)
                .append_pair("redirect_to", &callback_url)
                .append_pair("code_challenge", &challenge)
                .append_pair("code_challenge_method", "s256")
                .append_pair("state", &state);
            url
        };
        let attempt = PkceAttempt { verifier, state };
        store_secret(
            PKCE_ACCOUNT,
            &serde_json::to_string(&attempt)
                .map_err(|error| format!("Could not prepare secure sign-in: {error}"))?,
        )?;
        *self.pending_pkce.lock() = Some(attempt);
        open_url(url.as_str())?;
        let mut status = self.status();
        status.message = "Finish signing in in your browser".into();
        *self.cached_status.lock() = status.clone();
        Ok(status)
    }

    pub async fn complete_sign_in(&self, callback: &str) -> Result<AccountStatus, String> {
        self.require_config()?;
        let url =
            Url::parse(callback).map_err(|error| format!("Invalid sign-in callback: {error}"))?;
        if url.scheme() != "henlocal" || url.host_str() != Some("auth") || url.path() != "/callback"
        {
            return Err("Unknown Hen Local sign-in callback".into());
        }
        if let Some(error) = url
            .query_pairs()
            .find(|(key, _)| key == "error_description")
        {
            return Err(error.1.into_owned());
        }
        let code = query_value(&url, "code")
            .ok_or_else(|| "Sign-in callback is missing its code".to_string())?;
        let callback_state = query_value(&url, "state");
        let attempt = self
            .pending_pkce
            .lock()
            .take()
            .or_else(|| {
                keychain_secret(PKCE_ACCOUNT)
                    .ok()
                    .flatten()
                    .and_then(|value| serde_json::from_str(&value).ok())
            })
            .ok_or_else(|| {
                "This sign-in attempt is no longer active. Start sign-in again.".to_string()
            })?;
        if callback_state.as_deref() != Some(attempt.state.as_str()) {
            return Err("Sign-in state did not match. Start sign-in again.".into());
        }
        delete_secret(PKCE_ACCOUNT)?;
        let response = self
            .http
            .post(format!(
                "{}/auth/v1/token?grant_type=pkce",
                self.config.supabase_url
            ))
            .header("apikey", &self.config.anon_key)
            .json(&json!({ "auth_code": code, "code_verifier": attempt.verifier }))
            .send()
            .await
            .map_err(network_error)?;
        let token: TokenResponse = response_json(response).await?;
        let session = StoredSession {
            access_token: token.access_token,
            refresh_token: token.refresh_token,
            expires_at: Utc::now().timestamp() + token.expires_in,
            email: token.user.and_then(|user| user.email).unwrap_or_default(),
        };
        save_session(&session)?;
        self.refresh().await
    }

    pub async fn refresh(&self) -> Result<AccountStatus, String> {
        if !self.config.configured() {
            return Ok(self.status());
        }
        let Some(mut session) = load_session()? else {
            self.update_local_status();
            return Ok(self.status());
        };
        if session.expires_at <= Utc::now().timestamp() + 300 {
            session = self.refresh_session(&session).await?;
        }
        let mut billing = self
            .api_json(Method::GET, "billing-api/status", &session, None)
            .await?;
        let mut device_response = self
            .api_json(Method::GET, "license-api/devices", &session, None)
            .await?;
        let lease = load_lease().ok().flatten();
        let lease_result = lease
            .as_ref()
            .and_then(|lease| verify_lease(lease, &device_public_key().ok()?).ok());
        let should_refresh_lease = lease_result
            .as_ref()
            .and_then(|payload| parse_date(&payload.expires_at).ok())
            .map(|expires| expires <= Utc::now() + ChronoDuration::hours(24))
            .unwrap_or(true);
        let mut license_message = None;
        if should_refresh_lease {
            if let Some(device_id) = lease_result
                .as_ref()
                .map(|payload| payload.device_id.clone())
            {
                self.refresh_lease(&session, &device_id).await?;
            } else {
                if let Err(error) = self.activate_device(&session).await {
                    if error.contains("DEVICE_LIMIT_REACHED") {
                        license_message = Some(
                            "Two Macs are already active. Deactivate one below, then refresh this license."
                                .to_string(),
                        );
                    } else {
                        return Err(error);
                    }
                } else {
                    billing = self
                        .api_json(Method::GET, "billing-api/status", &session, None)
                        .await?;
                    device_response = self
                        .api_json(Method::GET, "license-api/devices", &session, None)
                        .await?;
                }
            }
        }
        let valid_payload = load_lease()
            .ok()
            .flatten()
            .and_then(|lease| verify_lease(&lease, &device_public_key().ok()?).ok());
        let current_device_id = valid_payload
            .as_ref()
            .map(|payload| payload.device_id.clone());
        let devices = serde_json::from_value::<Vec<RawDevice>>(
            device_response
                .get("devices")
                .cloned()
                .unwrap_or_else(|| json!([])),
        )
        .map_err(|error| format!("Invalid device list: {error}"))?
        .into_iter()
        .map(|device| AccountDevice {
            current: current_device_id.as_deref() == Some(device.id.as_str()),
            id: device.id,
            friendly_name: device.friendly_name,
            activated_at: device.activated_at,
            last_used_at: device.last_used_at,
            deactivated_at: device.deactivated_at,
        })
        .collect();
        let entitlement = billing.get("entitlement").cloned().unwrap_or(Value::Null);
        let subscription = billing.get("subscription").cloned().unwrap_or(Value::Null);
        let status = AccountStatus {
            configured: true,
            signed_in: true,
            email: (!session.email.is_empty()).then_some(session.email),
            subscription_state: string_field(&subscription, "state", "unknown"),
            entitlement_source: string_field(&entitlement, "source", "unknown"),
            access_until: optional_string_field(&entitlement, "accessUntil"),
            license_valid: valid_payload.is_some(),
            lease_expires_at: valid_payload
                .as_ref()
                .map(|payload| payload.expires_at.clone()),
            current_device_id,
            devices,
            message: license_message.unwrap_or_else(|| {
                if valid_payload.is_some() {
                    "License is ready for offline translation".into()
                } else {
                    "Connect to refresh this device license".into()
                }
            }),
        };
        *self.cached_status.lock() = status.clone();
        Ok(status)
    }

    pub async fn checkout(&self) -> Result<(), String> {
        let session = self.valid_session().await?;
        let body = self
            .api_json(
                Method::POST,
                "billing-api/checkout",
                &session,
                Some(json!({})),
            )
            .await?;
        open_returned_url(&body)
    }

    pub async fn portal(&self) -> Result<(), String> {
        let session = self.valid_session().await?;
        let body = self
            .api_json(
                Method::POST,
                "billing-api/portal",
                &session,
                Some(json!({})),
            )
            .await?;
        open_returned_url(&body)
    }

    pub async fn deactivate_device(&self, device_id: &str) -> Result<AccountStatus, String> {
        let session = self.valid_session().await?;
        self.api_json(
            Method::POST,
            "license-api/deactivate",
            &session,
            Some(json!({ "deviceId": device_id })),
        )
        .await?;
        let current = load_lease()
            .ok()
            .flatten()
            .and_then(|lease| verify_lease(&lease, &device_public_key().ok()?).ok())
            .map(|payload| payload.device_id == device_id)
            .unwrap_or(false);
        if current {
            let _ = fs::remove_file(lease_path());
            delete_secret(SESSION_ACCOUNT)?;
            self.update_local_status();
            return Ok(self.status());
        }
        self.refresh().await
    }

    pub fn sign_out(&self) -> Result<AccountStatus, String> {
        delete_secret(SESSION_ACCOUNT)?;
        let _ = fs::remove_file(lease_path());
        self.update_local_status();
        Ok(self.status())
    }

    pub fn translation_allowed(&self) -> Result<(), String> {
        if !self.config.configured() {
            return Ok(());
        }
        let device_key = device_public_key()?;
        let lease = load_lease()?.ok_or_else(|| {
            "Sign in and activate this Mac before starting live translation".to_string()
        })?;
        verify_lease(&lease, &device_key).map(|_| ())
    }

    async fn valid_session(&self) -> Result<StoredSession, String> {
        self.require_config()?;
        let session = load_session()?.ok_or_else(|| "Sign in first".to_string())?;
        if session.expires_at <= Utc::now().timestamp() + 300 {
            self.refresh_session(&session).await
        } else {
            Ok(session)
        }
    }

    async fn refresh_session(&self, session: &StoredSession) -> Result<StoredSession, String> {
        let response = self
            .http
            .post(format!(
                "{}/auth/v1/token?grant_type=refresh_token",
                self.config.supabase_url
            ))
            .header("apikey", &self.config.anon_key)
            .json(&json!({ "refresh_token": session.refresh_token }))
            .send()
            .await
            .map_err(network_error)?;
        let token: TokenResponse = response_json(response).await?;
        let refreshed = StoredSession {
            access_token: token.access_token,
            refresh_token: token.refresh_token,
            expires_at: Utc::now().timestamp() + token.expires_in,
            email: token
                .user
                .and_then(|user| user.email)
                .filter(|email| !email.is_empty())
                .unwrap_or_else(|| session.email.clone()),
        };
        save_session(&refreshed)?;
        Ok(refreshed)
    }

    async fn activate_device(&self, session: &StoredSession) -> Result<(), String> {
        let public_key = device_public_key()?;
        let fingerprint = hex_digest(Sha256::digest(public_key.as_bytes()).as_slice());
        let response = self
            .api_json(
                Method::POST,
                "license-api/activate",
                session,
                Some(json!({
                    "fingerprint": fingerprint,
                    "publicKey": public_key,
                    "friendlyName": friendly_device_name()
                })),
            )
            .await?;
        save_lease_from_response(&response)
    }

    async fn refresh_lease(&self, session: &StoredSession, device_id: &str) -> Result<(), String> {
        let response = self
            .api_json(
                Method::POST,
                "license-api/refresh",
                session,
                Some(json!({ "deviceId": device_id })),
            )
            .await?;
        save_lease_from_response(&response)
    }

    async fn api_json(
        &self,
        method: Method,
        route: &str,
        session: &StoredSession,
        body: Option<Value>,
    ) -> Result<Value, String> {
        let mut request = self
            .http
            .request(
                method,
                format!("{}/functions/v1/{route}", self.config.supabase_url),
            )
            .header("apikey", &self.config.anon_key)
            .bearer_auth(&session.access_token);
        if let Some(body) = body {
            request = request.json(&body);
        }
        response_json(request.send().await.map_err(network_error)?).await
    }

    fn require_config(&self) -> Result<(), String> {
        self.config
            .configured()
            .then_some(())
            .ok_or_else(|| "Account service is not configured in this internal build".to_string())
    }

    fn update_local_status(&self) {
        let mut status = self.cached_status.lock();
        status.configured = self.config.configured();
        if !status.configured {
            return;
        }
        match load_session() {
            Ok(Some(session)) => {
                status.signed_in = true;
                status.email = (!session.email.is_empty()).then_some(session.email);
            }
            _ => {
                status.signed_in = false;
                status.email = None;
                status.subscription_state = "signed_out".into();
                status.entitlement_source = "signed_out".into();
                status.access_until = None;
                status.devices.clear();
                status.message = "Sign in to start or refresh your license".into();
            }
        }
        let verified = load_lease()
            .ok()
            .flatten()
            .and_then(|lease| device_public_key().ok().map(|key| (key, lease)))
            .and_then(|(key, lease)| verify_lease(&lease, &key).ok());
        status.license_valid = verified.is_some();
        status.lease_expires_at = verified.as_ref().map(|payload| payload.expires_at.clone());
        status.current_device_id = verified.as_ref().map(|payload| payload.device_id.clone());
    }
}

#[derive(Debug, Deserialize)]
struct RawDevice {
    id: String,
    friendly_name: String,
    activated_at: String,
    last_used_at: String,
    deactivated_at: Option<String>,
}

fn random_urlsafe(bytes: usize) -> String {
    let mut value = vec![0_u8; bytes];
    OsRng.fill_bytes(&mut value);
    general_purpose::URL_SAFE_NO_PAD.encode(value)
}

fn query_value(url: &Url, key: &str) -> Option<String> {
    url.query_pairs()
        .find(|(candidate, _)| candidate == key)
        .map(|(_, value)| value.into_owned())
}

async fn response_json<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
) -> Result<T, String> {
    let status = response.status();
    let text = response.text().await.map_err(network_error)?;
    if !status.is_success() {
        let message = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|value| {
                value
                    .get("error_description")
                    .or_else(|| value.get("error"))
                    .or_else(|| value.get("message"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| format!("Account service returned {status}"));
        return Err(message);
    }
    serde_json::from_str(&text).map_err(|error| format!("Invalid account response: {error}"))
}

fn network_error(error: reqwest::Error) -> String {
    if error.is_timeout() {
        "Account service timed out. Check the network and try again.".into()
    } else {
        format!("Could not reach the account service: {error}")
    }
}

fn open_returned_url(body: &Value) -> Result<(), String> {
    let url = body
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| "Account service did not return a browser URL".to_string())?;
    open_url(url)
}

fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let status = Command::new("/usr/bin/open").arg(url).status();
    #[cfg(target_os = "windows")]
    let status = Command::new("cmd").args(["/C", "start", "", url]).status();
    #[cfg(all(unix, not(target_os = "macos")))]
    let status = Command::new("xdg-open").arg(url).status();
    status
        .map_err(|error| format!("Could not open the browser: {error}"))?
        .success()
        .then_some(())
        .ok_or_else(|| "Could not open the browser".into())
}

fn friendly_device_name() -> String {
    #[cfg(target_os = "macos")]
    if let Ok(output) = Command::new("/usr/sbin/scutil")
        .args(["--get", "ComputerName"])
        .output()
    {
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !name.is_empty() {
            return name.chars().take(100).collect();
        }
    }
    "Mac".into()
}

fn keychain_secret(account: &str) -> Result<Option<String>, String> {
    #[cfg(target_os = "macos")]
    {
        let entry = keyring::Entry::new(KEYCHAIN_SERVICE, account)
            .map_err(|error| format!("Could not open macOS Keychain: {error}"))?;
        match entry.get_password() {
            Ok(value) => return Ok(Some(value)),
            Err(keyring::Error::NoEntry) => return Ok(None),
            Err(error) => return Err(format!("Could not read macOS Keychain: {error}")),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = account;
        Ok(None)
    }
}

fn store_secret(account: &str, value: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return keyring::Entry::new(KEYCHAIN_SERVICE, account)
            .and_then(|entry| entry.set_password(value))
            .map_err(|error| format!("Could not write macOS Keychain: {error}"));
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (account, value);
        Err("Secure credential storage is not available on this platform".into())
    }
}

fn delete_secret(account: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let entry = keyring::Entry::new(KEYCHAIN_SERVICE, account)
            .map_err(|error| format!("Could not open macOS Keychain: {error}"))?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => return Ok(()),
            Err(error) => return Err(format!("Could not update macOS Keychain: {error}")),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = account;
        Ok(())
    }
}

fn save_session(session: &StoredSession) -> Result<(), String> {
    let serialized = serde_json::to_string(session)
        .map_err(|error| format!("Could not save account session: {error}"))?;
    store_secret(SESSION_ACCOUNT, &serialized)
}

fn load_session() -> Result<Option<StoredSession>, String> {
    keychain_secret(SESSION_ACCOUNT)?
        .map(|value| {
            serde_json::from_str(&value)
                .map_err(|error| format!("Stored account session is invalid: {error}"))
        })
        .transpose()
}

fn device_signing_key() -> Result<SigningKey, String> {
    if let Some(stored) = keychain_secret(DEVICE_KEY_ACCOUNT)? {
        let bytes = general_purpose::STANDARD
            .decode(stored)
            .map_err(|error| format!("Stored device key is invalid: {error}"))?;
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| "Stored device key has the wrong length".to_string())?;
        return Ok(SigningKey::from_bytes(&bytes));
    }
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let key = SigningKey::from_bytes(&bytes);
    store_secret(DEVICE_KEY_ACCOUNT, &general_purpose::STANDARD.encode(bytes))?;
    Ok(key)
}

fn device_public_key() -> Result<String, String> {
    Ok(general_purpose::STANDARD.encode(device_signing_key()?.verifying_key().as_bytes()))
}

fn lease_path() -> PathBuf {
    preferences::preferences_dir().join("license_lease.json")
}

fn load_lease() -> Result<Option<LicenseEnvelope>, String> {
    let path = lease_path();
    if !path.is_file() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)
        .map_err(|error| format!("Could not read the local license: {error}"))?;
    serde_json::from_str(&content)
        .map(Some)
        .map_err(|error| format!("The local license is corrupt: {error}"))
}

fn save_lease_from_response(response: &Value) -> Result<(), String> {
    let lease = response
        .get("lease")
        .cloned()
        .ok_or_else(|| "License service did not return a lease".to_string())?;
    let envelope: LicenseEnvelope = serde_json::from_value(lease)
        .map_err(|error| format!("License service returned an invalid lease: {error}"))?;
    let key = device_public_key()?;
    verify_lease(&envelope, &key)?;
    let path = lease_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create license directory: {error}"))?;
    }
    let temporary = path.with_extension("json.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(&envelope)
            .map_err(|error| format!("Could not serialize license: {error}"))?,
    )
    .map_err(|error| format!("Could not save license: {error}"))?;
    fs::rename(temporary, path).map_err(|error| format!("Could not finish saving license: {error}"))
}

fn verify_lease(
    envelope: &LicenseEnvelope,
    device_public_key: &str,
) -> Result<LicensePayload, String> {
    let public_key = general_purpose::STANDARD
        .decode(LICENSE_PUBLIC_KEY.trim())
        .map_err(|error| format!("Embedded license key is invalid: {error}"))?;
    let public_key: [u8; 32] = public_key
        .try_into()
        .map_err(|_| "Embedded license key has the wrong length".to_string())?;
    let verifying_key = VerifyingKey::from_bytes(&public_key)
        .map_err(|error| format!("Embedded license key is invalid: {error}"))?;
    let now = Utc::now();
    let payload = verify_lease_at(envelope, device_public_key, &verifying_key, now)?;
    enforce_monotonic_clock(now)?;
    Ok(payload)
}

fn enforce_monotonic_clock(now: DateTime<Utc>) -> Result<(), String> {
    let current = now.timestamp();
    if let Some(stored) = keychain_secret(LICENSE_CLOCK_ACCOUNT)? {
        let watermark = stored
            .parse::<i64>()
            .map_err(|_| "The secure license clock record is invalid".to_string())?;
        if current + ChronoDuration::minutes(5).num_seconds() < watermark {
            return Err(
                "The Mac clock moved backwards. Restore automatic date and time, then refresh the license."
                    .into(),
            );
        }
        if current <= watermark {
            return Ok(());
        }
    }
    store_secret(LICENSE_CLOCK_ACCOUNT, &current.to_string())
}

fn verify_lease_at(
    envelope: &LicenseEnvelope,
    device_public_key: &str,
    verifying_key: &VerifyingKey,
    now: DateTime<Utc>,
) -> Result<LicensePayload, String> {
    let signature = general_purpose::STANDARD
        .decode(&envelope.signature)
        .map_err(|error| format!("License signature is invalid: {error}"))?;
    let signature = Signature::from_slice(&signature)
        .map_err(|error| format!("License signature is invalid: {error}"))?;
    verifying_key
        .verify_strict(envelope.canonical_payload.as_bytes(), &signature)
        .map_err(|_| "The local license signature could not be verified".to_string())?;
    let canonical_payload: LicensePayload = serde_json::from_str(&envelope.canonical_payload)
        .map_err(|error| format!("License payload is invalid: {error}"))?;
    if canonical_payload != envelope.payload {
        return Err("The local license payload does not match its signature".into());
    }
    if canonical_payload.version != 1
        || canonical_payload.key_id != LICENSE_KEY_ID
        || canonical_payload.plan != "hen-local-monthly"
        || !canonical_payload.unlimited_local_translation
    {
        return Err("The local license is not supported by this app version".into());
    }
    if canonical_payload.device_public_key != device_public_key {
        return Err("This license belongs to a different Mac".into());
    }
    let issued = parse_date(&canonical_payload.issued_at)?;
    let expires = parse_date(&canonical_payload.expires_at)?;
    if issued > now + ChronoDuration::minutes(5) {
        return Err("The Mac clock is earlier than the license issue time".into());
    }
    if expires <= now {
        return Err("This device license has expired. Connect to refresh it.".into());
    }
    if expires > issued + ChronoDuration::days(7) + ChronoDuration::minutes(1) {
        return Err("The license is longer than the supported offline window".into());
    }
    Ok(canonical_payload)
}

fn parse_date(value: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| format!("License date is invalid: {error}"))
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn string_field(value: &Value, field: &str, fallback: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .to_string()
}

fn optional_string_field(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::Signer;

    fn signed_envelope(
        signing_key: &SigningKey,
        device_key: &str,
        expires: DateTime<Utc>,
    ) -> LicenseEnvelope {
        let issued = Utc::now() - ChronoDuration::seconds(1);
        let payload = LicensePayload {
            version: 1,
            key_id: LICENSE_KEY_ID.into(),
            user_id: "user-1".into(),
            device_id: "device-1".into(),
            device_public_key: device_key.into(),
            issued_at: issued.to_rfc3339(),
            expires_at: expires.to_rfc3339(),
            plan: "hen-local-monthly".into(),
            unlimited_local_translation: true,
            source: "trial".into(),
        };
        let canonical_payload = serde_json::to_string(&payload).unwrap();
        LicenseEnvelope {
            signature: general_purpose::STANDARD
                .encode(signing_key.sign(canonical_payload.as_bytes()).to_bytes()),
            payload,
            canonical_payload,
        }
    }

    #[test]
    fn rejects_tampered_license_before_dates_are_trusted() {
        let mut random = [0_u8; 32];
        OsRng.fill_bytes(&mut random);
        let signing_key = SigningKey::from_bytes(&random);
        let device = "device-public-key";
        let mut envelope =
            signed_envelope(&signing_key, device, Utc::now() + ChronoDuration::days(1));
        envelope.payload.device_id = "tampered".into();
        assert!(
            verify_lease_at(&envelope, device, &signing_key.verifying_key(), Utc::now()).is_err()
        );
    }

    #[test]
    fn rejects_license_for_another_device() {
        let mut random = [0_u8; 32];
        OsRng.fill_bytes(&mut random);
        let signing_key = SigningKey::from_bytes(&random);
        let envelope = signed_envelope(
            &signing_key,
            "device-a",
            Utc::now() + ChronoDuration::days(1),
        );
        assert!(verify_lease_at(
            &envelope,
            "device-b",
            &signing_key.verifying_key(),
            Utc::now()
        )
        .is_err());
    }

    #[test]
    fn accepts_valid_seven_day_device_bound_license() {
        let mut random = [0_u8; 32];
        OsRng.fill_bytes(&mut random);
        let signing_key = SigningKey::from_bytes(&random);
        let now = Utc::now();
        let envelope = signed_envelope(
            &signing_key,
            "device-a",
            now + ChronoDuration::days(7) - ChronoDuration::seconds(2),
        );
        assert!(verify_lease_at(&envelope, "device-a", &signing_key.verifying_key(), now).is_ok());
    }

    #[test]
    fn rejects_expired_license() {
        let mut random = [0_u8; 32];
        OsRng.fill_bytes(&mut random);
        let signing_key = SigningKey::from_bytes(&random);
        let envelope = signed_envelope(
            &signing_key,
            "device-a",
            Utc::now() - ChronoDuration::seconds(1),
        );
        assert!(verify_lease_at(
            &envelope,
            "device-a",
            &signing_key.verifying_key(),
            Utc::now()
        )
        .is_err());
    }

    #[test]
    fn rejects_a_signed_lease_for_an_unknown_plan() {
        let mut random = [0_u8; 32];
        OsRng.fill_bytes(&mut random);
        let signing_key = SigningKey::from_bytes(&random);
        let mut envelope = signed_envelope(
            &signing_key,
            "device-a",
            Utc::now() + ChronoDuration::days(1),
        );
        envelope.payload.plan = "unknown-plan".into();
        envelope.canonical_payload = serde_json::to_string(&envelope.payload).unwrap();
        envelope.signature = general_purpose::STANDARD.encode(
            signing_key
                .sign(envelope.canonical_payload.as_bytes())
                .to_bytes(),
        );
        assert!(verify_lease_at(
            &envelope,
            "device-a",
            &signing_key.verifying_key(),
            Utc::now()
        )
        .is_err());
    }
}
