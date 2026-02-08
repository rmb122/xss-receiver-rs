use std::sync::Arc;

use axum::{
    extract::FromRef,
    http::{header::AUTHORIZATION, request::Parts},
    response::IntoResponse,
};
use axum_extra::extract::CookieJar;
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::utils::response::Response;

const AUTHORIZATION_PREFIX: &str = "Bearer ";

/// A generic struct for holding the claims of a JWT token.
#[derive(Debug, Deserialize)]
pub struct Claims<T>(pub T);

#[derive(Debug, Serialize, Deserialize)]
pub struct ExpirablePayload<T> {
    exp: i64,

    #[serde(flatten)]
    payload: T,
}

impl<S, T> axum::extract::FromRequestParts<S> for Claims<T>
where
    Arc<JwtManager>: FromRef<S>,
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // 先从 headers 里面获取
        let mut token = if let Some(token) = parts.headers.get(AUTHORIZATION) {
            token.to_str().map_err(|_| AuthError {})?.to_owned()
        } else {
            let jar = CookieJar::from_headers(&parts.headers);
            if let Some(token) = jar.get(AUTHORIZATION.as_str()) {
                token.value_trimmed().to_owned()
            } else {
                return Err(AuthError {});
            }
        };

        if token.trim().starts_with(AUTHORIZATION_PREFIX) {
            token = token[AUTHORIZATION_PREFIX.len()..].trim().to_owned()
        }

        let decoder = Arc::<JwtManager>::from_ref(state);
        let token_data: TokenData<ExpirablePayload<T>> =
            decoder.decode(&token).map_err(|_| AuthError {})?;

        Ok(Claims(token_data.claims.payload))
    }
}

pub struct AuthError;

impl IntoResponse for AuthError {
    fn into_response(self) -> axum::http::Response<axum::body::Body> {
        Response::<()>::new()
            .code(400)
            .msg("token verify failed")
            .into_response()
    }
}

#[derive(Clone)]
pub struct JwtManager {
    algorithm: Algorithm,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    expire_time: i64,
    validation: Validation,
}

impl JwtManager {
    pub fn new(algorithm: Algorithm, key: &[u8], expire_time: i64) -> Self {
        return JwtManager {
            algorithm,
            encoding_key: EncodingKey::from_secret(key),
            decoding_key: DecodingKey::from_secret(key),
            expire_time: expire_time,
            validation: Validation::new(algorithm),
        };
    }

    pub fn decode<T: DeserializeOwned>(&self, token: &str) -> Result<TokenData<T>, AuthError> {
        if let Ok(payload) = jsonwebtoken::decode::<T>(token, &self.decoding_key, &self.validation)
        {
            return Ok(payload);
        }
        return Err(AuthError {});
    }

    pub fn encode_token<T: Serialize>(&self, claim: T) -> anyhow::Result<String> {
        return Ok(jsonwebtoken::encode(
            &Header::new(self.algorithm),
            &ExpirablePayload {
                exp: (Utc::now() + Duration::seconds(self.expire_time)).timestamp(),
                payload: claim,
            },
            &self.encoding_key,
        )?);
    }
}
