use std::{collections::HashMap, sync::Mutex};

use centaurus::{
  bail,
  error::{ErrorReportStatusExt, Result},
  eyre::Context,
};
use chrono::Utc;
use hmac::{KeyInit, Mac};
use http::{HeaderMap, StatusCode};
use rand::Rng;
use sha2::{Digest, Sha256};

type HmacSha3_512 = hmac::Hmac<sha2::Sha512>;

const TIMESTAMP_HEADER: &str = "x-wings-timestamp";
const NONCE_HEADER: &str = "x-wings-nonce";
const SIGNATURE_HEADER: &str = "x-wings-signature";

/// Maximum allowed clock difference between signer and verifier.
pub const MAX_SKEW_MS: i64 = 30_000;

/// A signed request: method, path (with query) and body hash are covered by
/// the signature, so a captured signature can not be reused for another request.
pub struct SignData {
  method: String,
  path: String,
  body_hash: String,
  timestamp: i64,
  nonce: String,
}

impl SignData {
  pub fn new(method: &str, path: &str, body: &[u8]) -> Self {
    Self {
      method: method.to_string(),
      path: path.to_string(),
      body_hash: hex::encode(Sha256::digest(body)),
      timestamp: Utc::now().timestamp_millis(),
      nonce: random_nonce(),
    }
  }

  fn canonical(&self) -> String {
    format!(
      "{}\n{}\n{}\n{}\n{}",
      self.method, self.path, self.body_hash, self.timestamp, self.nonce
    )
  }

  pub fn add_to_header_map(&self, headers: &mut HeaderMap, token: &str) -> Result<()> {
    let signature = hmac(&self.canonical(), token)?;
    headers.insert(NONCE_HEADER, self.nonce.parse().unwrap());
    headers.insert(TIMESTAMP_HEADER, self.timestamp.into());
    headers.insert(SIGNATURE_HEADER, signature.parse().unwrap());
    Ok(())
  }

  /// Validates a signed request. Rejects stale timestamps; nonce reuse has to
  /// be checked separately with a [`NonceCache`].
  pub fn validate_header_map(
    headers: &HeaderMap,
    token: &str,
    method: &str,
    path: &str,
    body: &[u8],
  ) -> Result<Self> {
    let timestamp: i64 = get_header_value(headers, TIMESTAMP_HEADER)?
      .parse()
      .status_context(StatusCode::UNAUTHORIZED, "Invalid wings timestamp")?;
    if (Utc::now().timestamp_millis() - timestamp).abs() > MAX_SKEW_MS {
      bail!(UNAUTHORIZED, "Expired wings timestamp");
    }

    let data = SignData {
      method: method.to_string(),
      path: path.to_string(),
      body_hash: hex::encode(Sha256::digest(body)),
      timestamp,
      nonce: get_header_value(headers, NONCE_HEADER)?,
    };
    verify(
      &data.canonical(),
      token,
      &get_header_value(headers, SIGNATURE_HEADER)?,
    )?;

    Ok(data)
  }

  /// Signs the response to this request. The signature covers the request, so
  /// it can not be replayed as the response to a different request.
  pub fn add_response_to_header_map(&self, headers: &mut HeaderMap, token: &str) -> Result<()> {
    let nonce = random_nonce();
    let signature = hmac(&format!("{}\n{}", self.canonical(), nonce), token)?;
    headers.insert(NONCE_HEADER, nonce.parse().unwrap());
    headers.insert(SIGNATURE_HEADER, signature.parse().unwrap());
    Ok(())
  }

  pub fn validate_response_header_map(&self, headers: &HeaderMap, token: &str) -> Result<()> {
    let nonce = get_header_value(headers, NONCE_HEADER)?;
    let signature = get_header_value(headers, SIGNATURE_HEADER)?;
    verify(
      &format!("{}\n{}", self.canonical(), nonce),
      token,
      &signature,
    )
  }
}

/// Remembers nonces for as long as their timestamp is accepted.
// ponytail: in-memory, per-process; fine for a single wings instance
#[derive(Default)]
pub struct NonceCache {
  seen: Mutex<HashMap<String, i64>>,
}

impl NonceCache {
  pub fn check(&self, data: &SignData) -> Result<()> {
    let now = Utc::now().timestamp_millis();
    let mut seen = self.seen.lock().unwrap();
    // anything older is rejected by the timestamp check anyway
    seen.retain(|_, ts| now - *ts <= MAX_SKEW_MS);

    if seen.insert(data.nonce.clone(), data.timestamp).is_some() {
      bail!(UNAUTHORIZED, "Reused wings nonce");
    }
    Ok(())
  }
}

fn random_nonce() -> String {
  let mut rng = rand::rng();
  let mut raw_nonce = [0u8; 16];
  rng.fill_bytes(&mut raw_nonce);
  hex::encode(raw_nonce)
}

fn get_header_value(headers: &HeaderMap, key: &str) -> Result<String> {
  let value = headers
    .get(key)
    .status_context(StatusCode::UNAUTHORIZED, &format!("Missing {} header", key))?
    .to_str()
    .status_context(StatusCode::UNAUTHORIZED, &format!("Invalid {} header", key))?
    .to_string();
  Ok(value)
}

fn mac(data: &str, key: &str) -> Result<HmacSha3_512> {
  let mut mac = HmacSha3_512::new_from_slice(key.as_bytes()).context("hamc error")?;
  mac.update(data.as_bytes());
  Ok(mac)
}

fn hmac(data: &str, key: &str) -> Result<String> {
  Ok(hex::encode(mac(data, key)?.finalize().into_bytes()))
}

fn verify(data: &str, key: &str, signature: &str) -> Result<()> {
  let Ok(signature) = hex::decode(signature) else {
    bail!(UNAUTHORIZED, "Invalid wings signature");
  };
  if mac(data, key)?.verify_slice(&signature).is_err() {
    bail!(UNAUTHORIZED, "Invalid wings signature");
  }
  Ok(())
}

#[cfg(test)]
mod test {
  use super::*;

  const TOKEN: &str = "token";

  fn signed(method: &str, path: &str, body: &[u8]) -> (SignData, HeaderMap) {
    let data = SignData::new(method, path, body);
    let mut headers = HeaderMap::new();
    data.add_to_header_map(&mut headers, TOKEN).unwrap();
    (data, headers)
  }

  #[test]
  fn roundtrip() {
    let (_, headers) = signed("POST", "/api/deploy", b"body");
    SignData::validate_header_map(&headers, TOKEN, "POST", "/api/deploy", b"body").unwrap();
  }

  #[test]
  fn rejects_tampering() {
    let (_, headers) = signed("POST", "/api/deploy", b"body");
    let check = |token, method, path, body: &[u8]| {
      SignData::validate_header_map(&headers, token, method, path, body).is_err()
    };
    assert!(check("other", "POST", "/api/deploy", b"body"));
    assert!(check(TOKEN, "GET", "/api/deploy", b"body"));
    assert!(check(TOKEN, "POST", "/api/other", b"body"));
    assert!(check(TOKEN, "POST", "/api/deploy", b"other"));
  }

  #[test]
  fn rejects_stale_timestamp() {
    let mut data = SignData::new("GET", "/api", b"");
    data.timestamp -= MAX_SKEW_MS + 1000;
    let mut headers = HeaderMap::new();
    data.add_to_header_map(&mut headers, TOKEN).unwrap();
    assert!(SignData::validate_header_map(&headers, TOKEN, "GET", "/api", b"").is_err());
  }

  #[test]
  fn rejects_reused_nonce() {
    let (_, headers) = signed("GET", "/api", b"");
    let cache = NonceCache::default();
    let data = SignData::validate_header_map(&headers, TOKEN, "GET", "/api", b"").unwrap();
    cache.check(&data).unwrap();
    let data = SignData::validate_header_map(&headers, TOKEN, "GET", "/api", b"").unwrap();
    assert!(cache.check(&data).is_err());
  }

  #[test]
  fn response_is_bound_to_request() {
    let (request, _) = signed("GET", "/api", b"");
    let (other, _) = signed("GET", "/api", b"");
    let mut headers = HeaderMap::new();
    request
      .add_response_to_header_map(&mut headers, TOKEN)
      .unwrap();

    request
      .validate_response_header_map(&headers, TOKEN)
      .unwrap();
    assert!(other.validate_response_header_map(&headers, TOKEN).is_err());
  }
}
