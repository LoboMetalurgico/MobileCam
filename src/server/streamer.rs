//! Server-side logic for handling streamer-specific WebSocket messages.

use actix_ws::{CloseCode, CloseReason};
use prost::bytes::Buf;

use crate::{
  app_state::AppState,
  proto::{
    common::{
      IceCandidate, RequestRtcAnswer, Session as ProtoSession, SessionType, UpdateVideoTransform,
      VideoTransform,
    },
    controller::{
      BatteryLevel, DropSession, NewSession, UpdateZoom, new_session::Session as NewSessionEnum,
    },
    streamer::{client_to_server::Command as ClientCommand, decode_client_to_server},
    viewer::DisconnectStreamer,
  },
};

// Handle a new connection from a streamer session, performing any necessary setup or initialization for the session.
pub async fn handle_connection(
  session_id: usize,
  app_data: &AppState,
) -> Result<(), Option<CloseReason>> {
  if let Some(myself) = app_data.get_proto_streamer_session(session_id) {
    for (i, mut controller_session) in app_data.get_all_controller_sessions() {
      if let Err(e) = controller_session
        .send_command(NewSession {
          session: Some(NewSessionEnum::StreamerSession(myself.clone())),
        })
        .await
      {
        tracing::warn!(
          "Failed to send new streamer session {session_id} notification to controller session {i}: {e}"
        );
      }
    }
  } else {
    tracing::error!("Failed to find streamer session {session_id}");
  }

  Ok(())
}

/// Handle a disconnection from a streamer session, performing any necessary cleanup and notifying relevant viewer and controller sessions about the disconnection.
pub async fn handle_disconnect(session_id: usize, app_data: &AppState) {
  for (i, mut controller_session) in app_data.get_all_controller_sessions() {
    if let Err(e) = controller_session
      .send_command(DropSession {
        session: Some(ProtoSession {
          id: session_id as u64,
          r#type: SessionType::Streamer as i32,
        }),
      })
      .await
    {
      tracing::warn!(
        "Failed to send streamer disconnect notification to controller session {i}: {e}"
      );
    }
  }

  for (i, mut viewer_session) in app_data.handle_streamer_disconnection(session_id) {
    if let Err(e) = viewer_session.send_command(DisconnectStreamer {}).await {
      tracing::warn!("Failed to send streamer disconnect notification to viewer session {i}: {e}");
    }
  }
}

/// Handle a message from a streamer session, processing the binary data and performing necessary actions based on the message content.
pub async fn handle_message(
  session_id: usize,
  app_data: &AppState,
  message: impl Buf,
) -> Result<(), Option<CloseReason>> {
  let client_to_server = decode_client_to_server(message).map_err(|e| {
    tracing::warn!("Failed to decode message from streamer: {e}");

    Some(CloseReason {
      code: CloseCode::Unsupported,
      description: Some("Failed to decode message".into()),
    })
  })?;

  match client_to_server {
    ClientCommand::RtcOfferResponse(command) => {
      let Some(video_transform) = app_data
        .get_streamer_video_transform(session_id)
        .map(|v| v.map(VideoTransform::from))
      else {
        tracing::warn!("Failed to find streamer session");

        return Err(Some(CloseReason {
          code: CloseCode::Error,
          description: Some("Failed to find streamer session for RTC answer response".into()),
        }));
      };

      let rtc_answer = RequestRtcAnswer {
        offer: command.offer,
        streamer_id: session_id as u64,
        video_transform,
      };

      match command.session {
        Some(session) if session.r#type == SessionType::Controller as i32 => {
          if let Some(mut controller_session) = app_data.get_controller_session(session.id as usize)
          {
            tracing::debug!(
              "Received RTC answer response for controller session {:?}",
              command.session
            );

            if let Err(e) = controller_session.send_command(rtc_answer).await {
              tracing::warn!(
                "Failed to send RTC answer response to controller session {}: {e}",
                session.id
              );
            }

            Ok(())
          } else {
            tracing::warn!(
              "Controller session with ID {} not found for RTC answer response",
              session.id
            );

            Ok(())
          }
        }

        Some(session) if session.r#type == SessionType::Viewer as i32 => {
          if let Some(mut viewer_session) = app_data.get_viewer_session(session.id as usize) {
            tracing::debug!(
              "Received RTC answer response for viewer session {:?}",
              command.session
            );

            if let Err(e) = viewer_session.send_command(rtc_answer).await {
              tracing::warn!(
                "Failed to send RTC answer response to viewer session {}: {e}",
                session.id
              );
            }

            Ok(())
          } else {
            tracing::warn!(
              "Viewer session with ID {} not found for RTC answer response",
              session.id
            );

            Ok(())
          }
        }

        _ => {
          tracing::warn!("Received RTC answer response with invalid or missing session");

          Ok(())
        }
      }
    }

    ClientCommand::RequestVideoTransform(command) => {
      if !app_data.set_streamer_video_transform(session_id, command.video_transform.map(Into::into))
      {
        tracing::warn!("Failed to find streamer session for video transform request");

        return Err(Some(CloseReason {
          code: CloseCode::Error,
          description: Some("Failed to find streamer session for video transform request".into()),
        }));
      }

      let update_video_transform = UpdateVideoTransform {
        streamer_id: session_id as u64,
        video_transform: command.video_transform,
      };

      for (viewer_id, mut viewer_session) in
        app_data.get_all_viewer_sessions_by_streamer(Some(session_id))
      {
        if let Err(e) = viewer_session.send_command(update_video_transform).await {
          tracing::warn!(
            "Failed to send video transform update to viewer session {viewer_id}: {e}",
          );
        }
      }

      for (controller_id, mut controller_session) in app_data.get_all_controller_sessions() {
        if let Err(e) = controller_session
          .send_command(update_video_transform)
          .await
        {
          tracing::warn!(
            "Failed to send video transform update to controller session {controller_id}: {e}",
          );
        }
      }

      Ok(())
    }

    ClientCommand::UpdateBatteryLevel(command) => {
      if !app_data.set_streamer_battery_level(session_id, Some(command.battery_level as u8)) {
        tracing::warn!("Failed to find streamer session for battery level update");

        return Err(Some(CloseReason {
          code: CloseCode::Error,
          description: Some("Failed to find streamer session for battery level update".into()),
        }));
      }

      let battery_level = BatteryLevel {
        streamer_id: session_id as u64,
        battery_level: command.battery_level,
      };

      for (controller_id, mut controller_session) in app_data.get_all_controller_sessions() {
        if let Err(e) = controller_session.send_command(battery_level).await {
          tracing::warn!(
            "Failed to send battery level update to controller session {controller_id}: {e}",
          );
        }
      }

      Ok(())
    }

    ClientCommand::IceCandidate(command) => {
      tracing::debug!("Received ICE candidate from streamer session {session_id}");

      let ice_candidate = IceCandidate {
        candidate: command.candidate,
        session: Some(ProtoSession {
          id: session_id as u64,
          r#type: SessionType::Streamer as i32,
        }),
      };

      match command.session {
        Some(session) if session.r#type == SessionType::Controller as i32 => {
          if let Some(mut controller_session) = app_data.get_controller_session(session.id as usize)
          {
            if let Err(e) = controller_session.send_command(ice_candidate).await {
              tracing::warn!(
                "Failed to send ICE candidate to controller session {}: {e}",
                session.id
              );
            }

            Ok(())
          } else {
            tracing::warn!(
              "Controller session with ID {} not found for ICE candidate",
              session.id
            );

            Ok(())
          }
        }

        Some(session) if session.r#type == SessionType::Viewer as i32 => {
          if let Some(mut viewer_session) = app_data.get_viewer_session(session.id as usize) {
            if let Err(e) = viewer_session.send_command(ice_candidate).await {
              tracing::warn!(
                "Failed to send ICE candidate to viewer session {}: {e}",
                session.id
              );
            }

            Ok(())
          } else {
            tracing::warn!(
              "Viewer session with ID {} not found for ICE candidate",
              session.id
            );

            Ok(())
          }
        }

        _ => {
          tracing::warn!("Received ICE candidate with invalid or missing session");

          Ok(())
        }
      }
    }

    ClientCommand::RequestZoom(command) => {
      tracing::debug!("Received zoom request from streamer session {session_id}");

      let zoom = UpdateZoom {
        streamer_id: session_id as u64,
        zoom: command.zoom,
      };

      for (controller_id, mut controller_session) in app_data.get_all_controller_sessions() {
        if let Err(e) = controller_session.send_command(zoom).await {
          tracing::warn!("Failed to send zoom request to controller session {controller_id}: {e}",);
        }
      }

      Ok(())
    }
  }
}
