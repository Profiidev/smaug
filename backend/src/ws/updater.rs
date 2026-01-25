use axum::{
  Router,
  extract::{
    WebSocketUpgrade,
    ws::{Message, WebSocket},
  },
  response::IntoResponse,
  routing::any,
};
use futures_util::StreamExt;
use tokio::sync::mpsc::Receiver;
use uuid::Uuid;

use crate::ws::state::{UpdateMessage, UpdateState};

pub fn router() -> Router {
  Router::new().route("/updater", any(update))
}

async fn update(ws: WebSocketUpgrade, state: UpdateState) -> impl IntoResponse {
  let (uuid, recv) = state.create_session().await;

  ws.on_upgrade(move |socket| handle_socket(socket, uuid, recv, state))
}

async fn handle_socket(
  mut socket: WebSocket,
  uuid: Uuid,
  mut recv: Receiver<UpdateMessage>,
  state: UpdateState,
) {
  loop {
    tokio::select! {
      update = recv.recv() => {
        match update {
          Some(message) => {
            let message = serde_json::to_string(&message).unwrap();
            let message = Message::Text(message.into());

            let _ = socket.send(message).await;
          }
          None => {
            state.remove_session(&uuid).await;
            break;
          }
        }
      }

      ws_msg = socket.next() => {
        if let Some(Ok(Message::Close(_)) | Err(_)) | None = ws_msg {
          state.remove_session(&uuid).await;
          break;
        }
      }
    }
  }
}
