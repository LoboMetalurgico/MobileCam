//! This module contains the protobuf definitions for the streamer, as well as helper functions to encode and decode messages.

use actix_ws::{Closed, Session};
use prost::{Message, bytes::Buf};

use crate::proto::{
  DecodeError, common,
  streamer::{
    client_to_server::Command as ClientCommand, server_to_client::Command as ServerCommand,
  },
};

include!(concat!(env!("OUT_DIR"), "/streamer.rs"));

// Decodes a [`ClientToServer`] message from the given buffer, and extracts the command from it.
pub fn decode_client_to_server(buf: impl Buf) -> Result<ClientCommand, DecodeError> {
  ClientToServer::decode(buf)
    .map_err(From::from)
    .and_then(|v| v.command.ok_or(DecodeError::MissingCommand))
}

/// Encodes a [`ServerToClient`] message into a byte vector.
pub fn encode_server_to_client<C: IntoServerToClient>(command: C) -> Vec<u8> {
  command.into_server_to_client().encode_to_vec()
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
pub struct StreamerSession(Session);

impl StreamerSession {
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

impl IntoServerToClient for RequestRtcOffer {
  fn into_server_to_client(self) -> ServerToClient {
    ServerCommand::RequestRtcOffer(self).into_server_to_client()
  }
}

impl IntoServerToClient for RtcAnswer {
  fn into_server_to_client(self) -> ServerToClient {
    ServerCommand::RtcAnswer(self).into_server_to_client()
  }
}

impl IntoServerToClient for DisconnectPeer {
  fn into_server_to_client(self) -> ServerToClient {
    ServerCommand::DisconnectPeer(self).into_server_to_client()
  }
}

impl IntoServerToClient for common::IceCandidate {
  fn into_server_to_client(self) -> ServerToClient {
    ServerCommand::IceCandidate(self).into_server_to_client()
  }
}

impl IntoServerToClient for Zoom {
  fn into_server_to_client(self) -> ServerToClient {
    ServerCommand::ChangeZoom(self).into_server_to_client()
  }
}

impl IntoServerToClient for ChangeQuality {
  fn into_server_to_client(self) -> ServerToClient {
    ServerCommand::ChangeQuality(self).into_server_to_client()
  }
}
