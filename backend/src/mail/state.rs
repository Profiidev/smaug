use std::{
  sync::Arc,
  thread::{sleep, spawn},
  time::{Duration, Instant},
};

use axum::{Extension, extract::FromRequestParts};
use centaurus::{
  bail,
  db::init::Connection,
  error::{ErrorReportStatusExt, Result},
  eyre::Context,
};
use dashmap::DashMap;
use http::StatusCode;
use lettre::{
  AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
  message::{Mailbox, header::ContentType},
  transport::smtp::authentication::Credentials,
};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::db::{
  DBTrait,
  settings::{MailSettings, SmtpSettings},
};

#[derive(Clone, FromRequestParts)]
#[from_request(via(Extension))]
pub struct Mailer(Arc<Mutex<Option<MailConfig>>>);

struct MailConfig {
  sender: Mailbox,
  transport: AsyncSmtpTransport<Tokio1Executor>,
}

impl Mailer {
  pub async fn new(db: &Connection) -> Self {
    let state = Mailer(Arc::new(Mutex::new(None)));
    let settings: MailSettings = db.settings().get_settings().await.unwrap_or_default();
    if let Some(smtp_config) = settings.smtp {
      state.try_init(&smtp_config).await.ok();
    }
    state
  }

  pub async fn try_init(&self, smtp_config: &SmtpSettings) -> Result<()> {
    let mut guard = self.0.lock().await;
    if guard.is_none() {
      let config = MailConfig::new(smtp_config)?;
      *guard = Some(config);
    }
    Ok(())
  }

  pub async fn deactivate(&self) {
    let mut guard = self.0.lock().await;
    *guard = None;
  }

  pub async fn is_active(&self) -> bool {
    let guard = self.0.lock().await;
    guard.is_some()
  }

  pub async fn send_mail(
    &self,
    username: String,
    email: String,
    subject: String,
    body: String,
  ) -> Result<()> {
    let lock = self.0.lock().await;
    if let Some(config) = &*lock {
      config.send_mail(username, email, subject, body).await
    } else {
      bail!("Mail service is not configured");
    }
  }
}

impl MailConfig {
  fn new(smtp_config: &SmtpSettings) -> Result<Self> {
    let credentials = Credentials::new(smtp_config.username.clone(), smtp_config.password.clone());

    let relay = if smtp_config.use_tls {
      AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp_config.server)
    } else {
      AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp_config.server)
    };
    let transport = relay
      .status_context(StatusCode::BAD_REQUEST, "Failed to create SMTP transport")?
      .port(smtp_config.port)
      .credentials(credentials)
      .build();

    let sender = Mailbox::new(
      Some(smtp_config.from_name.clone()),
      smtp_config
        .from_address
        .clone()
        .parse()
        .status_context(StatusCode::NOT_ACCEPTABLE, "Invalid from address")?,
    );

    Ok(MailConfig { sender, transport })
  }

  pub async fn send_mail(
    &self,
    username: String,
    email: String,
    subject: String,
    body: String,
  ) -> Result<()> {
    let receiver = Mailbox::new(
      Some(username),
      email.parse().with_context(|| "Invalid email")?,
    );

    let mail = Message::builder()
      .from(self.sender.clone())
      .to(receiver)
      .subject(subject)
      .header(ContentType::TEXT_HTML)
      .body(body)?;

    self
      .transport
      .send(mail)
      .await
      .with_context(|| "Failed to send email")?;

    Ok(())
  }
}

#[derive(FromRequestParts, Clone)]
#[from_request(via(Extension))]
pub struct ResetPasswordState {
  tokens: Arc<DashMap<String, (String, Instant)>>,
}

impl ResetPasswordState {
  pub async fn generate_token(&self, email: String) -> String {
    let token = Uuid::new_v4().to_string();
    self.tokens.insert(token.clone(), (email, Instant::now()));
    token
  }

  pub async fn validate_token(&self, token: &str) -> Option<String> {
    self.tokens.get(token).map(|entry| entry.value().clone().0)
  }

  pub async fn invalidate_token(&self, email: &str) {
    self.tokens.remove(email);
  }
}

impl Default for ResetPasswordState {
  fn default() -> Self {
    let map = Arc::new(DashMap::new());

    spawn({
      let map = map.clone();
      move || {
        loop {
          sleep(Duration::from_secs(600));
          let now = Instant::now();
          map.retain(|_, &mut (_, timestamp)| {
            now.duration_since(timestamp) < Duration::from_secs(3600)
          });
        }
      }
    });

    ResetPasswordState { tokens: map }
  }
}
