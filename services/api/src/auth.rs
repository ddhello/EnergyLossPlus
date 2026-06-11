use base64::Engine;
use chrono::{Duration, Utc};
use passkey_auth::{
    Attachment, AuthenticationResponse, AuthenticationState, CredentialId, PasskeyCredential,
    RegistrationResponse, RegistrationState, Webauthn,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub token: String,
    pub user_id: String,
    pub nickname: String,
    pub device_name: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserAccount {
    pub user_id: Uuid,
    pub nickname: String,
    pub device_name: String,
    pub passkeys: Vec<PasskeyCredential>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeRecord {
    pub challenge_id: String,
    pub nickname: String,
    pub device_name: Option<String>,
    pub user_id: Uuid,
    pub purpose: ChallengePurpose,
    pub state: ChallengeState,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ChallengePurpose {
    Register,
    Recover,
    Login,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChallengeState {
    Register(RegistrationState),
    Login(AuthenticationState),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicKeyChallenge {
    pub challenge_id: String,
    pub public_key: serde_json::Value,
}

#[derive(Clone)]
pub struct PasskeyService {
    webauthn: Webauthn,
}

impl PasskeyService {
    pub fn from_env() -> anyhow::Result<Self> {
        let rp_id = std::env::var("WEBAUTHN_RP_ID").unwrap_or_else(|_| "localhost".to_string());
        let rp_name =
            std::env::var("WEBAUTHN_RP_NAME").unwrap_or_else(|_| "EnergyLossPlus".to_string());
        let origin = std::env::var("WEBAUTHN_ORIGIN")
            .unwrap_or_else(|_| "http://localhost:1420".to_string());
        Ok(Self::new(&rp_id, &rp_name, &origin))
    }

    pub fn new(rp_id: &str, rp_name: &str, origin: &str) -> Self {
        let webauthn = Webauthn::new(rp_id, rp_name, origin)
            .strict_base64(true)
            .require_user_verification(true)
            .authenticator_attachment(Attachment::Any);
        Self { webauthn }
    }

    pub fn start_registration(
        &self,
        nickname: String,
        device_name: String,
        existing: &[CredentialId],
    ) -> anyhow::Result<(ChallengeRecord, PublicKeyChallenge)> {
        let user_id = Uuid::new_v4();
        let (challenge, state) =
            self.webauthn
                .start_registration(user_id.as_bytes(), &nickname, &nickname, existing);
        let record = ChallengeRecord {
            challenge_id: Uuid::new_v4().to_string(),
            nickname,
            device_name: Some(device_name),
            user_id,
            purpose: ChallengePurpose::Register,
            state: ChallengeState::Register(state),
            expires_at: expires_in_five_minutes(),
        };
        Ok((
            record.clone(),
            json_challenge(record.challenge_id, challenge)?,
        ))
    }

    pub fn start_recovery(
        &self,
        account: &UserAccount,
        device_name: String,
    ) -> anyhow::Result<(ChallengeRecord, PublicKeyChallenge)> {
        let existing = account
            .passkeys
            .iter()
            .map(|passkey| passkey.id.clone())
            .collect::<Vec<_>>();
        let (challenge, state) = self.webauthn.start_registration(
            account.user_id.as_bytes(),
            &account.nickname,
            &account.nickname,
            &existing,
        );
        let record = ChallengeRecord {
            challenge_id: Uuid::new_v4().to_string(),
            nickname: account.nickname.clone(),
            device_name: Some(device_name),
            user_id: account.user_id,
            purpose: ChallengePurpose::Recover,
            state: ChallengeState::Register(state),
            expires_at: expires_in_five_minutes(),
        };
        Ok((
            record.clone(),
            json_challenge(record.challenge_id, challenge)?,
        ))
    }

    pub fn finish_registration(
        &self,
        credential: serde_json::Value,
        record: ChallengeRecord,
    ) -> anyhow::Result<UserAccount> {
        ensure_not_expired(&record)?;
        let ChallengeState::Register(state) = record.state else {
            anyhow::bail!("challenge is not a registration ceremony");
        };
        let response = registration_response_from_credential(credential)?;
        let passkey = self.webauthn.finish_registration(&state, &response)?;
        let now = Utc::now().to_rfc3339();
        Ok(UserAccount {
            user_id: record.user_id,
            nickname: record.nickname,
            device_name: record.device_name.unwrap_or_else(|| "desktop".to_string()),
            passkeys: vec![passkey],
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn finish_recovery(
        &self,
        credential: serde_json::Value,
        record: ChallengeRecord,
        mut account: UserAccount,
    ) -> anyhow::Result<UserAccount> {
        ensure_not_expired(&record)?;
        if record.purpose != ChallengePurpose::Recover || record.user_id != account.user_id {
            anyhow::bail!("challenge is not an account recovery ceremony");
        }
        let ChallengeState::Register(state) = record.state else {
            anyhow::bail!("challenge is not a registration ceremony");
        };
        let response = registration_response_from_credential(credential)?;
        let passkey = self.webauthn.finish_registration(&state, &response)?;
        account.passkeys = vec![passkey];
        account.device_name = record.device_name.unwrap_or_else(|| "desktop".to_string());
        account.updated_at = Utc::now().to_rfc3339();
        Ok(account)
    }

    pub fn start_login(
        &self,
        account: &UserAccount,
    ) -> anyhow::Result<(ChallengeRecord, PublicKeyChallenge)> {
        let (challenge, state) = self.webauthn.start_authentication_with_creds_for_user(
            account.user_id.as_bytes(),
            &account.passkeys,
        );
        let record = ChallengeRecord {
            challenge_id: Uuid::new_v4().to_string(),
            nickname: account.nickname.clone(),
            device_name: Some(account.device_name.clone()),
            user_id: account.user_id,
            purpose: ChallengePurpose::Login,
            state: ChallengeState::Login(state),
            expires_at: expires_in_five_minutes(),
        };
        Ok((
            record.clone(),
            json_challenge(record.challenge_id, challenge)?,
        ))
    }

    pub fn finish_login(
        &self,
        credential: serde_json::Value,
        record: ChallengeRecord,
        mut account: UserAccount,
    ) -> anyhow::Result<UserAccount> {
        ensure_not_expired(&record)?;
        let ChallengeState::Login(state) = record.state else {
            anyhow::bail!("challenge is not an authentication ceremony");
        };
        let response = authentication_response_from_credential(credential)?;
        let Some(index) = account
            .passkeys
            .iter()
            .position(|passkey| passkey.id.to_b64url() == response.id)
        else {
            anyhow::bail!("authenticated credential is not registered to this account");
        };

        let outcome =
            self.webauthn
                .finish_authentication(&state, &response, &account.passkeys[index])?;
        account.passkeys[index].counter = outcome.new_counter;
        account.updated_at = Utc::now().to_rfc3339();
        Ok(account)
    }
}

pub fn create_session(account: &UserAccount) -> Session {
    Session {
        token: random_urlsafe_token(32),
        user_id: account.user_id.to_string(),
        nickname: account.nickname.clone(),
        device_name: account.device_name.clone(),
        expires_at: (Utc::now() + Duration::hours(12)).to_rfc3339(),
    }
}

fn random_urlsafe_token(bytes: usize) -> String {
    let mut buffer = vec![0; bytes];
    rand::thread_rng().fill_bytes(&mut buffer);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buffer)
}

fn json_challenge<T>(challenge_id: String, challenge: T) -> anyhow::Result<PublicKeyChallenge>
where
    T: Serialize,
{
    Ok(PublicKeyChallenge {
        challenge_id,
        public_key: serde_json::to_value(challenge)?,
    })
}

fn registration_response_from_credential(
    value: serde_json::Value,
) -> anyhow::Result<RegistrationResponse> {
    let id = credential_id(&value)?;
    let response = response_object(&value)?;
    Ok(RegistrationResponse {
        id,
        transports: transports(&value),
        attestation_object: string_field(response, "attestationObject")?,
        client_data_json: string_field(response, "clientDataJSON")?,
    })
}

fn authentication_response_from_credential(
    value: serde_json::Value,
) -> anyhow::Result<AuthenticationResponse> {
    let id = credential_id(&value)?;
    let response = response_object(&value)?;
    Ok(AuthenticationResponse {
        id,
        authenticator_data: string_field(response, "authenticatorData")?,
        signature: string_field(response, "signature")?,
        client_data_json: string_field(response, "clientDataJSON")?,
        user_handle: optional_string_field(response, "userHandle"),
    })
}

fn credential_id(value: &serde_json::Value) -> anyhow::Result<String> {
    value
        .get("rawId")
        .or_else(|| value.get("id"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("credential id is missing"))
}

fn response_object(value: &serde_json::Value) -> anyhow::Result<&serde_json::Value> {
    value
        .get("response")
        .ok_or_else(|| anyhow::anyhow!("credential response is missing"))
}

fn string_field(value: &serde_json::Value, key: &str) -> anyhow::Result<String> {
    optional_string_field(value, key).ok_or_else(|| anyhow::anyhow!("{key} is missing"))
}

fn optional_string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn transports(value: &serde_json::Value) -> Vec<String> {
    value
        .get("transports")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn expires_in_five_minutes() -> String {
    (Utc::now() + Duration::minutes(5)).to_rfc3339()
}

fn ensure_not_expired(record: &ChallengeRecord) -> anyhow::Result<()> {
    let expires_at = chrono::DateTime::parse_from_rfc3339(&record.expires_at)?;
    if expires_at < Utc::now() {
        anyhow::bail!("challenge expired");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_account() -> UserAccount {
        let now = Utc::now().to_rfc3339();
        UserAccount {
            user_id: Uuid::new_v4(),
            nickname: "tester".to_string(),
            device_name: "desktop".to_string(),
            passkeys: vec![],
            created_at: now.clone(),
            updated_at: now,
        }
    }

    #[test]
    fn starts_registration_with_server_side_state() {
        let service = PasskeyService::new("localhost", "EnergyLossPlus", "http://localhost:1420");
        let (record, challenge) = service
            .start_registration("tester".into(), "desktop".into(), &[])
            .unwrap();

        assert_eq!(record.purpose, ChallengePurpose::Register);
        assert!(!challenge.challenge_id.is_empty());
        assert!(challenge.public_key.get("challenge").is_some());
        assert!(matches!(record.state, ChallengeState::Register(_)));
    }

    #[test]
    fn starts_recovery_for_the_existing_user_id() {
        let service = PasskeyService::new("localhost", "EnergyLossPlus", "http://localhost:1420");
        let account = test_account();
        let (record, _) = service
            .start_recovery(&account, "new phone".into())
            .unwrap();

        assert_eq!(record.purpose, ChallengePurpose::Recover);
        assert_eq!(record.user_id, account.user_id);
        assert_eq!(record.nickname, account.nickname);
    }

    #[test]
    fn flattens_browser_registration_credential() {
        let credential = serde_json::json!({
            "id": "fallback",
            "rawId": "raw",
            "response": {
                "attestationObject": "att",
                "clientDataJSON": "client"
            },
            "transports": ["internal"]
        });

        let response = registration_response_from_credential(credential).unwrap();
        assert_eq!(response.id, "raw");
        assert_eq!(response.attestation_object, "att");
        assert_eq!(response.client_data_json, "client");
        assert_eq!(response.transports, vec!["internal"]);
    }

    #[test]
    fn flattens_browser_authentication_credential() {
        let credential = serde_json::json!({
            "id": "auth-id",
            "response": {
                "authenticatorData": "auth-data",
                "clientDataJSON": "client",
                "signature": "sig",
                "userHandle": "user"
            }
        });

        let response = authentication_response_from_credential(credential).unwrap();
        assert_eq!(response.id, "auth-id");
        assert_eq!(response.authenticator_data, "auth-data");
        assert_eq!(response.client_data_json, "client");
        assert_eq!(response.signature, "sig");
        assert_eq!(response.user_handle.as_deref(), Some("user"));
    }

    #[test]
    fn creates_high_entropy_url_safe_sessions() {
        let account = test_account();
        let first = create_session(&account);
        let second = create_session(&account);

        assert_ne!(first.token, second.token);
        assert!(first.token.len() >= 43);
        assert!(!first.token.contains('='));
        assert!(first
            .token
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || char == '-' || char == '_'));
        assert!(Uuid::parse_str(&first.token).is_err());
    }
}
