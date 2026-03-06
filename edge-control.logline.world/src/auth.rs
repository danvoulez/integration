use axum::http::{header, HeaderMap};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;

use crate::{config::Config, models::ErrorResponseV1};

#[derive(Clone, Debug)]
pub struct AuthContext {
    pub subject: String,
    pub app_id: Option<String>,
    pub tenant_id: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JwtClaims {
    sub: String,
    #[allow(dead_code)]
    exp: usize,
    #[serde(default)]
    app_metadata: AppMetadata,
    #[serde(default)]
    tenant_id: Option<String>,
    #[serde(default)]
    role: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct AppMetadata {
    #[serde(default)]
    app_id: Option<String>,
    #[serde(default)]
    tenant_id: Option<String>,
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(ToString::to_string)
}

fn decode_supabase_hs256(token: &str, config: &Config) -> Option<AuthContext> {
    let secret = config.supabase_jwt_secret.as_deref()?;

    let mut validation = Validation::new(Algorithm::HS256);
    if let Some(aud) = &config.supabase_jwt_audience {
        validation.set_audience(&[aud]);
    } else {
        validation.validate_aud = false;
    }

    let decoded = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .ok()?;
    let claims = decoded.claims;

    let app_id = if claims.role.as_deref() == Some("service") {
        Some(claims.sub.clone())
    } else {
        claims.app_metadata.app_id
    };

    let tenant_id = if claims.role.as_deref() == Some("service") {
        claims.tenant_id
    } else {
        claims.app_metadata.tenant_id
    };

    Some(AuthContext {
        subject: claims.sub,
        app_id,
        tenant_id,
        role: claims.role,
    })
}

pub fn validate_headers(
    headers: &HeaderMap,
    config: &Config,
) -> Result<AuthContext, ErrorResponseV1> {
    if !config.auth_is_configured() {
        return Err(ErrorResponseV1::new(
            None,
            "service_misconfigured",
            "No auth method configured on edge-control",
        ));
    }

    let token = bearer_token(headers)
        .ok_or_else(|| ErrorResponseV1::new(None, "unauthorized", "Missing bearer token"))?;

    if let Some(ctx) = decode_supabase_hs256(&token, config) {
        return Ok(ctx);
    }

    if config
        .internal_api_token
        .as_deref()
        .is_some_and(|expected| expected == token)
    {
        return Ok(AuthContext {
            subject: "internal-service".into(),
            app_id: Some("edge-control-internal".into()),
            tenant_id: None,
            role: Some("service".into()),
        });
    }

    Err(ErrorResponseV1::new(
        None,
        "unauthorized",
        "Invalid bearer token",
    ))
}
