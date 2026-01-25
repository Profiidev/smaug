use centaurus::{
  bail,
  error::{ErrorReportStatusExt, Result},
};
use chrono::Utc;
use hmac::Mac;
use http::{HeaderMap, StatusCode};
use rand::RngCore;

type HmacSha3_512 = hmac::Hmac<sha3::Sha3_512>;

const TIMESTAMP_HEADER: &str = "x-wings-timestamp";
const NONCE_HEADER: &str = "x-wings-nonce";
const SIGNATURE_HEADER: &str = "x-wings-signature";

pub struct SignData {
  timestamp: String,
  nonce: String,
}

impl Default for SignData {
  fn default() -> Self {
    let timestamp = Utc::now().timestamp_millis().to_string();
    let nonce = random_nonce();
    Self { timestamp, nonce }
  }
}

impl SignData {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn from_timestamp(timestamp: String) -> Self {
    let nonce = random_nonce();
    Self { timestamp, nonce }
  }

  fn signature(&self, token: &str) -> Result<String> {
    let data = format!("{}.{}", self.timestamp, self.nonce);
    hmac(&data, token)
  }

  pub fn add_to_header_map(&self, headers: &mut HeaderMap, token: &str) -> Result<()> {
    let signature = self.signature(token)?;
    headers.insert(NONCE_HEADER, self.nonce.parse().unwrap());
    headers.insert(TIMESTAMP_HEADER, self.timestamp.parse().unwrap());
    headers.insert(SIGNATURE_HEADER, signature.parse().unwrap());
    Ok(())
  }

  pub fn to_header_map(&self, token: &str) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    self.add_to_header_map(&mut headers, token)?;
    Ok(headers)
  }

  fn validate(&self, token: &str, signature: &str) -> Result<()> {
    let correct_signature = self.signature(token)?;
    if correct_signature != signature {
      bail!(UNAUTHORIZED, "Invalid wings signature");
    }
    Ok(())
  }

  pub fn validate_header_map(
    headers: &HeaderMap,
    token: &str,
    initial_data: Option<Self>,
  ) -> Result<String> {
    let timestamp = get_header_value(headers, TIMESTAMP_HEADER)?;
    let nonce = get_header_value(headers, NONCE_HEADER)?;
    let signature = get_header_value(headers, SIGNATURE_HEADER)?;

    if let Some(data) = initial_data
      && data.nonce == nonce
    {
      bail!(UNAUTHORIZED, "Invalid wings nonce");
    }

    let sign_data = SignData { timestamp, nonce };
    sign_data.validate(token, &signature)?;

    Ok(sign_data.timestamp)
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

fn hmac(data: &str, key: &str) -> Result<String> {
  let mut mac = HmacSha3_512::new_from_slice(key.as_bytes())?;
  mac.update(data.as_bytes());
  Ok(hex::encode(mac.finalize().into_bytes()))
}
