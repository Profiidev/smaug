use std::sync::Arc;

use axum::{Extension, extract::FromRequestParts};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::{
  spawn,
  sync::mpsc::{self, Receiver, Sender},
  task::JoinHandle,
};
use tracing::debug;
use uuid::Uuid;

#[derive(Clone, FromRequestParts)]
#[from_request(via(Extension))]
pub struct UpdateState {
  sessions: DashMap<Uuid, Sender<UpdateMessage>>,
  #[allow(dead_code)]
  update_proxy: Arc<JoinHandle<()>>,
}

#[derive(Clone, FromRequestParts)]
#[from_request(via(Extension))]
pub struct Updater(Sender<UpdateMessage>);

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum UpdateMessage {
  Nodes,
  Settings,
  Users,
}

impl UpdateState {
  pub async fn init() -> (Self, Updater) {
    let sessions: DashMap<Uuid, Sender<UpdateMessage>> = DashMap::default();
    let (sender, mut receiver) = mpsc::channel(100);
    let updater = Updater(sender);

    let update_proxy = spawn({
      let sessions = sessions.clone();
      async move {
        while let Some(message) = receiver.recv().await {
          debug!("Broadcasting update message: {:?}", message);
          for pair in sessions.iter() {
            pair.value().send(message.clone()).await.ok();
          }
        }
      }
    });

    let state = Self {
      sessions,
      update_proxy: Arc::new(update_proxy),
    };

    (state, updater)
  }

  pub async fn create_session(&self) -> (Uuid, Receiver<UpdateMessage>) {
    let (send, recv) = mpsc::channel(100);
    let uuid = Uuid::new_v4();
    self.sessions.insert(uuid, send);

    (uuid, recv)
  }

  pub async fn remove_session(&self, uuid: &Uuid) {
    self.sessions.remove(uuid);
  }
}

impl Updater {
  pub async fn broadcast(&self, msg: UpdateMessage) {
    let _ = self.0.send(msg).await;
  }
}
