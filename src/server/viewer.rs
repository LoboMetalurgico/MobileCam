//! Server-side logic for handling viewer-specific WebSocket messages.

use actix_ws::{CloseCode, CloseReason};
use prost::bytes::Buf;

use crate::{
  app_state::AppState,
  proto::{
    common::{IceCandidate, Session as ProtoSession, SessionType},
    controller::{DropSession, NewSession, new_session::Session as NewSessionEnum},
    streamer::RtcAnswer,
    viewer::{client_to_server::Command as ClientCommand, decode_client_to_server},
  },
};

// Handle a new connection from a viewer session, performing any necessary setup or initialization for the session.
pub async fn handle_connection(
  session_id: usize,
  app_data: &AppState,
) -> Result<(), Option<CloseReason>> {
  if let Some(myself) = app_data.get_proto_viewer_session(session_id) {
    for (i, mut controller_session) in app_data.get_all_controller_sessions() {
      if let Err(e) = controller_session
        .send_command(NewSession {
          session: Some(NewSessionEnum::ViewerSession(myself.clone())),
        })
        .await
      {
        tracing::warn!(
          "Failed to send new viewer session {session_id} notification to controller session {i}: {e}"
        );
      }
    }
  } else {
    tracing::error!("Failed to find viewer session {session_id}");
  }

  Ok(())
}

/// Handle a disconnection from a viewer session, performing any necessary cleanup and notifying relevant controller sessions about the disconnection.
pub async fn handle_disconnect(session_id: usize, app_data: &AppState) {
  for (i, mut controller_session) in app_data.get_all_controller_sessions() {
    if let Err(e) = controller_session
      .send_command(DropSession {
        session: Some(ProtoSession {
          id: session_id as u64,
          r#type: SessionType::Viewer as i32,
        }),
      })
      .await
    {
      tracing::warn!(
        "Failed to send viewer disconnect notification for session {session_id} to controller session {i}: {e}"
      );
    }
  }
}

/// Handle a message from a viewer session, processing the binary data and performing necessary actions based on the message content.
pub async fn handle_message(
  session_id: usize,
  app_data: &AppState,
  message: impl Buf,
) -> Result<(), Option<CloseReason>> {
  let client_to_server = decode_client_to_server(message).map_err(|e| {
    tracing::warn!("Failed to decode message from viewer: {e}");

    Some(CloseReason {
      code: CloseCode::Unsupported,
      description: Some("Failed to decode message".into()),
    })
  })?;

  match client_to_server {
    ClientCommand::RtcAnswerResponse(command) => {
      if let Some(mut streamer_session) =
        app_data.get_streamer_session(command.streamer_id as usize)
      {
        if let Err(e) = streamer_session
          .send_command(RtcAnswer {
            session: Some(ProtoSession {
              id: session_id as u64,
              r#type: SessionType::Viewer as i32,
            }),
            answer: command.answer,
          })
          .await
        {
          tracing::warn!(
            "Failed to send RTC answer response to streamer session {}: {e}",
            command.streamer_id
          );
        }
      } else {
        tracing::warn!(
          "Failed to find streamer session {} for RTC answer response",
          command.streamer_id
        );
      }

      Ok(())
    }

    ClientCommand::IceCandidate(command) => {
      if let Some(streamer) = command.session
        && streamer.r#type == SessionType::Streamer as i32
        && let Some(mut streamer_session) = app_data.get_streamer_session(streamer.id as usize)
      {
        if let Err(e) = streamer_session
          .send_command(IceCandidate {
            session: Some(ProtoSession {
              id: session_id as u64,
              r#type: SessionType::Viewer as i32,
            }),
            candidate: command.candidate,
          })
          .await
        {
          tracing::warn!(
            "Failed to send ICE candidate to streamer session {}: {e}",
            streamer.id
          );
        }
      } else {
        tracing::warn!(
          "Failed to find streamer session or ICE candidate isn't for streamer session: {:?}",
          command.session
        );
      }

      Ok(())
    }
  }
}
