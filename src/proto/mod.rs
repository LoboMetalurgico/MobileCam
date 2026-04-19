//! Protobuf message definitions and encoding/decoding logic for the MobileCam application.

pub mod common;
pub mod controller;
pub mod streamer;
pub mod viewer;

/// Errors that can occur during decoding of messages.
#[derive(
  Debug, Clone, PartialEq, Eq, derive_more::From, derive_more::Error, derive_more::Display,
)]
pub enum DecodeError {
  /// An error occurred while decoding a Prost message.
  #[display("Prost decoding error: {_0}")]
  Prost(prost::DecodeError),

  /// The decoded message is missing the required `command` field.
  #[display("Missing command field")]
  MissingCommand,
}
