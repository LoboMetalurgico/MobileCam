//! Server-side logic for handling viewer-specific WebSocket messages.

use actix_ws::CloseReason;

/// Handle a message from a viewer session, processing the binary data and performing necessary actions based on the message content.
pub async fn handle_message() -> Result<(), Option<CloseReason>> {
  Ok(())
}
