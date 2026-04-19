//! This module contains the protobuf definitions for the controller, as well as helper functions to encode and decode messages.

use actix_ws::{Closed, Session};
use prost::{Message, bytes::Buf};

use crate::proto::{
  DecodeError, common,
  controller::{
    client_to_server::Command as ClientCommand, server_to_client::Command as ServerCommand,
  },
};

include!(concat!(env!("OUT_DIR"), "/controller.rs"));

/// Decodes a [`ClientToServer`] message from the given buffer, and extracts the command from it.
pub fn decode_client_to_server(buf: impl Buf) -> Result<ClientCommand, DecodeError> {
  ClientToServer::decode(buf)
    .map_err(From::from)
    .and_then(|v| v.command.ok_or(DecodeError::MissingCommand))
}

/// Encodes a [`ServerToClient`] message into a byte vector.
pub fn encode_server_to_client<C: IntoServerToClient>(command: C) -> Vec<u8> {
  command.into_server_to_client().encode_to_vec()
}

/// A wrapper around a mutable reference to a [`Session`] that provides helper methods for sending commands to the client.
#[derive(
  derive_more::From,
  derive_more::Deref,
  derive_more::DerefMut,
  derive_more::AsRef,
  derive_more::AsMut,
)]
pub struct ControllerSessionRef<'a>(&'a mut Session);

impl ControllerSessionRef<'_> {
  /// Sends a command to the client, encoding it as a [`ServerToClient`] message.
  pub async fn send_command<C: IntoServerToClient>(&mut self, command: C) -> Result<(), Closed> {
    self.0.binary(encode_server_to_client(command)).await
  }
}

/// A wrapper around [`Session`] that provides helper methods for sending commands to the client.
#[derive(
  Clone,
  derive_more::From,
  derive_more::Deref,
  derive_more::DerefMut,
  derive_more::AsRef,
  derive_more::AsMut,
)]
pub struct ControllerSession(Session);

impl ControllerSession {
  /// Sends a command to the client, encoding it as a [`ServerToClient`] message.
  pub async fn send_command<C: IntoServerToClient>(&mut self, command: C) -> Result<(), Closed> {
    self.0.binary(encode_server_to_client(command)).await
  }
}

/// A trait for types that can be converted into a [`ServerToClient`] message.
pub trait IntoServerToClient {
  /// Converts the type into a [`ServerToClient`] message.
  fn into_server_to_client(self) -> ServerToClient;
}

impl IntoServerToClient for ServerToClient {
  fn into_server_to_client(self) -> ServerToClient {
    self
  }
}

impl IntoServerToClient for ServerCommand {
  fn into_server_to_client(self) -> ServerToClient {
    ServerToClient {
      command: Some(self),
    }
  }
}

impl IntoServerToClient for common::RtcAnswer {
  fn into_server_to_client(self) -> ServerToClient {
    ServerCommand::RtcAnswer(self).into_server_to_client()
  }
}

impl IntoServerToClient for common::UpdateVideoTransform {
  fn into_server_to_client(self) -> ServerToClient {
    ServerCommand::UpdateVideoTransform(self).into_server_to_client()
  }
}

impl IntoServerToClient for BatteryLevel {
  fn into_server_to_client(self) -> ServerToClient {
    ServerCommand::BatteryLevel(self).into_server_to_client()
  }
}

impl IntoServerToClient for common::IceCandidate {
  fn into_server_to_client(self) -> ServerToClient {
    ServerCommand::IceCandidate(self).into_server_to_client()
  }
}

impl IntoServerToClient for UpdateZoom {
  fn into_server_to_client(self) -> ServerToClient {
    ServerCommand::UpdateZoom(self).into_server_to_client()
  }
}

impl IntoServerToClient for ListStreamersResponse {
  fn into_server_to_client(self) -> ServerToClient {
    ServerCommand::ListStreamersResponse(self).into_server_to_client()
  }
}

impl IntoServerToClient for ListViewersResponse {
  fn into_server_to_client(self) -> ServerToClient {
    ServerCommand::ListViewersResponse(self).into_server_to_client()
  }
}

impl IntoServerToClient for NewSession {
  fn into_server_to_client(self) -> ServerToClient {
    ServerCommand::NewSession(self).into_server_to_client()
  }
}

impl IntoServerToClient for DropSession {
  fn into_server_to_client(self) -> ServerToClient {
    ServerCommand::DropSession(self).into_server_to_client()
  }
}

impl IntoServerToClient for ChangedWatching {
  fn into_server_to_client(self) -> ServerToClient {
    ServerCommand::ChangedWatching(self).into_server_to_client()
  }
}
