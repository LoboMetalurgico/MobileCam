//! Server-side logic for handling controller-specific WebSocket messages.

use actix_ws::{CloseCode, CloseReason};
use prost::{Message, bytes::Buf};

use crate::{
  app_state::AppState,
  proto::{
    common::{IceCandidate, Quality, Session as ProtoSession, SessionType},
    controller::{
      ChangedWatching, ControllerSessionRef, ListStreamersResponse, ListViewersResponse, Ready,
      client_to_server::Command as ClientCommand, decode_client_to_server,
    },
    streamer::{ChangeQuality, RequestRtcOffer, RtcAnswer, Zoom},
    viewer::{DisconnectStreamer, UpdateMute},
  },
};

// Handle a new connection from a controller session, performing any necessary setup or initialization for the session.
pub async fn handle_connection(
  session_id: usize,
  mut controller_session: ControllerSessionRef<'_>,
  app_data: &AppState,
) -> Result<(), Option<CloseReason>> {
  if let Err(e) = controller_session
    .binary(
      Ready {
        id: session_id as u64,
        streamers: app_data.get_all_proto_streamer_sessions(),
        viewer_count: app_data.len_viewers() as u32,
      }
      .encode_to_vec(),
    )
    .await
  {
    tracing::warn!("Failed to send ready message to controller session {session_id}: {e}");

    Err(Some(CloseReason {
      code: CloseCode::Error,
      description: Some("Failed to send ready message".into()),
    }))
  } else {
    tracing::debug!("Sent ready message to controller session {session_id}");

    Ok(())
  }
}

/// Handle a message from a controller session, processing the binary data and performing necessary actions based on the message content.
pub async fn handle_message(
  session_id: usize,
  mut controller_session: ControllerSessionRef<'_>,
  app_data: &AppState,
  message: impl Buf,
) -> Result<(), Option<CloseReason>> {
  let client_to_server = decode_client_to_server(message).map_err(|e| {
    tracing::warn!("Failed to decode message from controller: {e}");

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
        tracing::debug!(
          "Received WebRTC answer response for streamer session {} with answer",
          command.streamer_id
        );

        streamer_session
          .send_command(RtcAnswer {
            session: Some(ProtoSession {
              id: session_id as u64,
              r#type: SessionType::Controller as i32,
            }),
            answer: command.answer,
          })
          .await
          .map_err(|e| {
            tracing::warn!("Failed to send WebRTC answer request to streamer session: {e}");
            Some(CloseReason {
              code: CloseCode::Error,
              description: Some("Failed to send WebRTC answer request".into()),
            })
          })?;

        Ok(())
      } else {
        tracing::warn!(
          "Streamer session with ID {} not found for WebRTC answer response",
          command.streamer_id
        );
        Ok(())
      }
    }

    ClientCommand::RequestChangeQuality(command) => {
      if let Some(mut streamer_session) =
        app_data.get_streamer_session(command.streamer_id as usize)
      {
        tracing::debug!(
          "Received change quality request for streamer session {} with quality {:?}",
          command.streamer_id,
          Quality::try_from(command.quality)
        );

        streamer_session
          .send_command(ChangeQuality {
            quality: command.quality,
          })
          .await
          .map_err(|e| {
            tracing::warn!("Failed to send change quality request to streamer session: {e}");
            Some(CloseReason {
              code: CloseCode::Error,
              description: Some("Failed to send change quality request".into()),
            })
          })?;

        Ok(())
      } else {
        tracing::warn!(
          "Streamer session with ID {} not found for change quality request",
          command.streamer_id
        );
        Ok(())
      }
    }

    ClientCommand::ListStreamers(_) => {
      tracing::debug!("Received request for list of streamers");

      controller_session
        .send_command(ListStreamersResponse {
          streamers: app_data.get_all_proto_streamer_sessions(),
        })
        .await
        .map_err(|e| {
          tracing::warn!("Failed to send list of streamers to controller session: {e}");
          Some(CloseReason {
            code: CloseCode::Error,
            description: Some("Failed to send list of streamers".into()),
          })
        })?;

      Ok(())
    }

    ClientCommand::ListViewers(command) => {
      tracing::debug!(
        "Received request for list of viewers for streamer session {:?}",
        command.streamer_id
      );

      controller_session
        .send_command(ListViewersResponse {
          viewers: app_data
            .get_proto_viewer_sessions_by_streamer(command.streamer_id.map(|id| id as usize)),
        })
        .await
        .map_err(|e| {
          tracing::warn!("Failed to send list of viewers to controller session: {e}");
          Some(CloseReason {
            code: CloseCode::Error,
            description: Some("Failed to send list of viewers".into()),
          })
        })?;

      Ok(())
    }

    ClientCommand::ListAllViewers(_) => {
      tracing::debug!("Received request for list of all viewers");

      controller_session
        .send_command(ListViewersResponse {
          viewers: app_data.get_all_proto_viewer_sessions(),
        })
        .await
        .map_err(|e| {
          tracing::warn!("Failed to send list of all viewers to controller session: {e}");
          Some(CloseReason {
            code: CloseCode::Error,
            description: Some("Failed to send list of all viewers".into()),
          })
        })?;

      Ok(())
    }

    ClientCommand::IceCandidate(command) => {
      if let Some(streamer_session_id) = command.session
        && streamer_session_id.r#type == SessionType::Streamer as i32
        && let Some(mut streamer_session) =
          app_data.get_streamer_session(streamer_session_id.id as usize)
      {
        tracing::debug!(
          "Received ICE candidate for streamer session {:?}",
          command.session
        );

        streamer_session
          .send_command(IceCandidate {
            candidate: command.candidate,
            session: Some(ProtoSession {
              id: session_id as u64,
              r#type: SessionType::Controller as i32,
            }),
          })
          .await
          .map_err(|e| {
            tracing::warn!(
              "Failed to send ICE candidate to streamer {}: {e}",
              streamer_session_id.id
            );

            Some(CloseReason {
              code: CloseCode::Error,
              description: Some("Failed to send ICE candidate".into()),
            })
          })?;

        Ok(())
      } else {
        tracing::warn!(
          "Streamer session with ID {:?} not found for ICE candidate",
          command.session
        );

        Ok(())
      }
    }

    ClientCommand::RequestZoom(command) => {
      if let Some(mut streamer_session) =
        app_data.get_streamer_session(command.streamer_id as usize)
      {
        tracing::debug!(
          "Received zoom level change request for streamer session {} with zoom level {}",
          command.streamer_id,
          command.zoom
        );

        streamer_session
          .send_command(Zoom { zoom: command.zoom })
          .await
          .map_err(|e| {
            tracing::warn!("Failed to send zoom level change request to streamer session: {e}");
            Some(CloseReason {
              code: CloseCode::Error,
              description: Some("Failed to send zoom level change request".into()),
            })
          })?;

        Ok(())
      } else {
        tracing::warn!(
          "Streamer session with ID {} not found for zoom level change request",
          command.streamer_id
        );

        Ok(())
      }
    }

    ClientCommand::UpdateWatching(command) => {
      if !app_data.update_viewer_watching(
        command.viewer_id as usize,
        command.streamer_id.map(|id| id as usize),
      ) {
        tracing::warn!(
          "Viewer session with ID {} not found for updating watching status",
          command.viewer_id
        );
      } else if let Some(streamer_id) = command.streamer_id {
        tracing::debug!(
          "Updated watching status for viewer session {} to watching streamer session {streamer_id}",
          command.viewer_id,
        );

        for (controller_id, mut controller_session) in app_data.get_all_controller_sessions() {
          if let Err(e) = controller_session
            .send_command(ChangedWatching {
              viewer_id: command.viewer_id,
              streamer_id: Some(streamer_id),
            })
            .await
          {
            tracing::warn!(
              "Failed to send updated watching status to controller session {controller_id}: {e}",
            );
          }
        }

        if let Some(mut streamer_session) = app_data.get_streamer_session(streamer_id as usize) {
          streamer_session
            .send_command(RequestRtcOffer {
              session: Some(ProtoSession {
                id: command.viewer_id,
                r#type: SessionType::Viewer as i32,
              }),
            })
            .await
            .map_err(|e| {
              tracing::warn!(
                "Failed to send updated viewer list to streamer session {}: {e}",
                streamer_id
              );
              Some(CloseReason {
                code: CloseCode::Error,
                description: Some("Failed to send updated viewer list".into()),
              })
            })?;
        }
      } else if let Some(mut viewer_session) =
        app_data.get_viewer_session(command.viewer_id as usize)
      {
        tracing::debug!(
          "Updated watching status for viewer session {} to not watching any streamer",
          command.viewer_id,
        );

        for (controller_id, mut controller_session) in app_data.get_all_controller_sessions() {
          if let Err(e) = controller_session
            .send_command(ChangedWatching {
              viewer_id: command.viewer_id,
              streamer_id: None,
            })
            .await
          {
            tracing::warn!(
              "Failed to send updated watching status to controller session {controller_id}: {e}",
            );
          }
        }

        viewer_session
          .send_command(DisconnectStreamer {})
          .await
          .map_err(|e| {
            tracing::warn!(
              "Failed to request viewer session {} to disconnect from streamer: {e}",
              command.viewer_id
            );
            Some(CloseReason {
              code: CloseCode::Error,
              description: Some("Failed to request viewer to disconnect from streamer".into()),
            })
          })?;
      }

      Ok(())
    }

    ClientCommand::RequestMute(command) => {
      if let Some(mut viewer_session) = app_data.get_viewer_session(command.viewer_id as usize) {
        tracing::debug!(
          "Received mute status change request with new mute status {}",
          command.muted
        );

        viewer_session
          .send_command(UpdateMute {
            muted: command.muted,
          })
          .await
          .map_err(|e| {
            tracing::warn!(
              "Failed to send mute status change request to viewer session {}: {e}",
              command.viewer_id
            );
            Some(CloseReason {
              code: CloseCode::Error,
              description: Some("Failed to send mute status change request".into()),
            })
          })?;

        Ok(())
      } else {
        tracing::warn!(
          "Viewer session with ID {} not found for mute status change request",
          command.viewer_id
        );

        Ok(())
      }
    }

    ClientCommand::RequestRtcOffer(command) => {
      if let Some(mut streamer_session) =
        app_data.get_streamer_session(command.streamer_id as usize)
      {
        if let Err(e) = streamer_session
          .send_command(RequestRtcOffer {
            session: Some(ProtoSession {
              id: session_id as u64,
              r#type: SessionType::Controller as i32,
            }),
          })
          .await
        {
          tracing::warn!(
            "Failed to send rtc request offer to streamer {}: {e}",
            command.streamer_id
          );
        }
      } else {
        tracing::warn!(
          "Streamer session with ID {} not found for request rtc offer",
          command.streamer_id
        );
      }

      Ok(())
    }
  }
}
