//! Bounded, allowlisted Google diagnostics. Never echo provider messages or URLs.
use crate::error::ApiError;
use serde_json::Value;
use uuid::Uuid;

fn diagnostic(body: &[u8]) -> String {
    let value: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let error = &value["error"];
    let mut parts = Vec::new();
    let mut reasons = Vec::new();
    if let Some(reason) = error.as_str() {
        reasons.push(reason);
    }
    if let Some(entries) = error["errors"].as_array() {
        reasons.extend(entries.iter().filter_map(|e| e["reason"].as_str()));
    }
    if let Some(entries) = error["details"].as_array() {
        for entry in entries {
            if entry["@type"] != "type.googleapis.com/google.rpc.ErrorInfo" {
                continue;
            }
            if let Some(reason) = entry["reason"].as_str() {
                reasons.push(reason);
            }
            let meta = &entry["metadata"];
            if let Some(service @ ("drive.googleapis.com" | "docs.googleapis.com")) =
                meta["service"].as_str()
            {
                parts.push(format!("service={service}"));
            }
            if let Some(consumer) = meta["consumer"].as_str() {
                if consumer.strip_prefix("projects/").is_some_and(|n| {
                    !n.is_empty() && n.len() <= 24 && n.bytes().all(|b| b.is_ascii_digit())
                }) {
                    parts.push(format!("consumer={consumer}"));
                }
            }
        }
    }
    for reason in reasons {
        let advice = match reason {
            "SERVICE_DISABLED" | "accessNotConfigured" => "Enable the service in the Cloud project owning the deployed OAuth client, not in each mailbox user's project",
            "ACCESS_TOKEN_SCOPE_INSUFFICIENT" | "insufficientPermissions" => "Reconnect the selected Google account and grant the required scope",
            "invalid_grant" | "authError" | "UNAUTHENTICATED" | "CREDENTIALS_MISSING" => "Reconnect the selected Google account; its authorization is invalid or expired",
            "invalid_client" | "unauthorized_client" => "Check the hosted application's OAuth client configuration",
            "forbidden" | "PERMISSION_DENIED" | "appNotAuthorizedToFile" | "domainPolicy" => "Check selected-account file access and Workspace administrator API policy",
            "rateLimitExceeded" | "userRateLimitExceeded" | "RATE_LIMIT_EXCEEDED" | "quotaExceeded" => "Check API quota and retry with backoff",
            _ => continue,
        };
        parts.push(format!("reason={reason}: {advice}"));
    }
    parts.sort();
    parts.dedup();
    if parts.is_empty() {
        "Google supplied no recognized diagnostic reason; no raw response was exposed".into()
    } else {
        parts.join("; ")
    }
}

pub(super) async fn failure(
    mut response: reqwest::Response,
    operation: &str,
    account_id: Uuid,
    client_id: &str,
) -> ApiError {
    let status = response.status().as_u16();
    let mut body = Vec::new();
    while let Ok(Some(chunk)) = response.chunk().await {
        if body.len() + chunk.len() > 16_384 {
            body.clear();
            break;
        }
        body.extend_from_slice(&chunk);
    }
    // Client IDs are public configuration; do not expose a misconfigured secret here.
    let client = if client_id.len() <= 256
        && client_id.ends_with(".apps.googleusercontent.com")
        && client_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._".contains(&b))
    {
        client_id
    } else {
        "unavailable"
    };
    let message = format!("Google {operation} failed (upstream HTTP {status}; account_id={account_id}; oauth_client_id={client}). {}", diagnostic(&body));
    if status == 401 || status == 403 {
        ApiError::Forbidden(message)
    } else {
        ApiError::Conflict(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinguishes_cloud_configuration_from_account_consent() {
        let disabled = diagnostic(br#"{"error":{"message":"SECRET","errors":[{"reason":"accessNotConfigured"}],"details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"SERVICE_DISABLED","metadata":{"consumer":"projects/123456","service":"drive.googleapis.com","activationUrl":"SECRET"}}]}}"#);
        assert!(disabled.contains("consumer=projects/123456"));
        assert!(disabled.contains("reason=SERVICE_DISABLED"));
        assert!(!disabled.contains("SECRET"));
        let scopes = diagnostic(br#"{"error":{"details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"ACCESS_TOKEN_SCOPE_INSUFFICIENT"}]}}"#);
        assert!(scopes.contains("Reconnect the selected Google account"));
    }
    #[test]
    fn suppresses_unrecognized_and_malformed_payloads() {
        for body in [b"<html>SECRET</html>".as_slice(), br#"{"error":{"message":"SECRET","errors":[{"reason":"SECRET"}],"details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","metadata":{"consumer":"projects/SECRET","service":"SECRET"}}]}}"#] {
            assert!(!diagnostic(body).contains("SECRET"));
        }
        assert!(
            diagnostic(br#"{"error":"invalid_grant","error_description":"SECRET"}"#)
                .contains("reason=invalid_grant")
        );
    }
}
