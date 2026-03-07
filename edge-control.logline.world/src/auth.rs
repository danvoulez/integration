use axum::http::{header, HeaderMap};
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, Validation};
use serde::Deserialize;

use crate::{models::ErrorResponseV1, AppState};

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

async fn decode_supabase_jwks(token: &str, state: &AppState) -> Option<AuthContext> {
    let jwks_url = state.config.supabase_jwks_url.as_deref()?;
    let header = decode_header(token).ok()?;
    let kid = header.kid?;

    let jwks = state
        .http_client
        .get(jwks_url)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json::<JwkSet>()
        .await
        .ok()?;
    let jwk = jwks
        .keys
        .into_iter()
        .find(|entry| entry.common.key_id == Some(kid.clone()))?;
    let key = DecodingKey::from_jwk(&jwk).ok()?;

    let mut validation = Validation::new(match header.alg {
        Algorithm::RS256 | Algorithm::RS384 | Algorithm::RS512 => header.alg,
        _ => Algorithm::RS256,
    });
    if let Some(aud) = &state.config.supabase_jwt_audience {
        validation.set_audience(&[aud]);
    } else {
        validation.validate_aud = false;
    }

    let decoded = decode::<JwtClaims>(token, &key, &validation).ok()?;
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

pub async fn validate_headers(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<AuthContext, ErrorResponseV1> {
    if !state.config.auth_is_configured() {
        return Err(ErrorResponseV1::new(
            None,
            "service_misconfigured",
            "No auth method configured on edge-control",
        ));
    }

    let token = bearer_token(headers)
        .ok_or_else(|| ErrorResponseV1::new(None, "unauthorized", "Missing bearer token"))?;

    if let Some(ctx) = decode_supabase_jwks(&token, state).await {
        return Ok(ctx);
    }

    if state
        .config
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
