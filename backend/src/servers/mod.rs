use aide::axum::{
  ApiRouter,
  routing::{get_with, post_with},
};

use self::{
  actions::{reinstall_server, set_server_installed, set_server_suspended, transfer_server},
  management::{
    create_server, delete_server, list_servers, server_info, server_options, update_server,
  },
};

mod access;
mod actions;
mod error;
mod management;
mod rules;
pub mod spec;
mod status;
mod types;
mod validation;

pub fn router() -> ApiRouter {
  ApiRouter::new()
    .api_route("/", get_with(list_servers, |op| op.id("list_servers")))
    .api_route("/", post_with(create_server, |op| op.id("create_server")))
    .api_route(
      "/options",
      get_with(server_options, |op| op.id("server_options")),
    )
    .api_route(
      "/{uuid}",
      get_with(server_info, |op| op.id("server_info"))
        .put_with(update_server, |op| op.id("update_server"))
        .delete_with(delete_server, |op| op.id("delete_server")),
    )
    .api_route(
      "/{uuid}/reinstall",
      post_with(reinstall_server, |op| op.id("reinstall_server")),
    )
    .api_route(
      "/{uuid}/install",
      post_with(set_server_installed, |op| op.id("set_server_installed")),
    )
    .api_route(
      "/{uuid}/suspend",
      post_with(set_server_suspended, |op| op.id("set_server_suspended")),
    )
    .api_route(
      "/{uuid}/transfer",
      post_with(transfer_server, |op| op.id("transfer_server")),
    )
}
