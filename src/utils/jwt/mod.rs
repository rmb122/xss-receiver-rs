use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Debug, Serialize, Deserialize)]
struct ExpirablePayload<T> {
    exp: i64,

    #[serde(flatten)]
    payload: T,
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

    pub fn decode_token<T: DeserializeOwned>(
        &self,
        token: &str,
    ) -> jsonwebtoken::errors::Result<T> {
        Ok(jsonwebtoken::decode::<ExpirablePayload<T>>(
            token,
            &self.decoding_key,
            &self.validation,
        )?
        .claims
        .payload)
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
