use std::{collections::HashMap, sync::Arc};

use axum::{Extension, extract::FromRequestParts};
use serde::{Deserialize, Serialize};
use tokio::{
  spawn,
  sync::{
    Mutex,
    mpsc::{self, Receiver, Sender},
  },
  task::JoinHandle,
};
use tracing::debug;
use uuid::Uuid;

pub type Sessions = Arc<Mutex<HashMap<Uuid, Sender<UpdateMessage>>>>;

#[derive(Clone, FromRequestParts)]
#[from_request(via(Extension))]
pub struct UpdateState {
  sessions: Sessions,
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
}

impl UpdateState {
  pub async fn init() -> (Self, Updater) {
    let sessions: Sessions = Default::default();
    let (sender, mut receiver) = mpsc::channel(100);
    let updater = Updater(sender);

    let update_proxy = spawn({
      let sessions = sessions.clone();
      async move {
        while let Some(message) = receiver.recv().await {
          debug!("Broadcasting update message: {:?}", message);
          for sender in sessions.lock().await.values() {
            sender.send(message.clone()).await.ok();
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

    let mut lock = self.sessions.lock().await;
    lock.insert(uuid, send);

    (uuid, recv)
  }

  pub async fn remove_session(&self, uuid: &Uuid) {
    self.sessions.lock().await.remove(uuid);
  }
}

impl Updater {
  pub async fn broadcast(&self, msg: UpdateMessage) {
    let _ = self.0.send(msg).await;
  }
}
