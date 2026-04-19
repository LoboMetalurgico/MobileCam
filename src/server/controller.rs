//! Server-side logic for handling controller-specific WebSocket messages.

use actix_ws::{CloseCode, CloseReason, Session};
use prost::{Message, bytes::Buf};

use crate::{
  app_state::{AppState, Role},
  proto::{
    common::{IceCandidate, Quality, Session as ProtoSession, SessionType},
    controller::{
      ClientToServer, ListStreamersResponse, ListViewersResponse, Ready, ServerToClient,
      client_to_server::Command as ClientCommand, server_to_client::Command as ServerCommand,
    },
    streamer::{
      ChangeQuality, CreateRtcOffer, ServerToClient as StreamerServerToClient, Zoom,
      server_to_client::Command as StreamerServerCommand,
    },
    viewer::{
      DisconnectStreamer, ServerToClient as ViewerServerToClient,
      server_to_client::Command as ViewerServerCommand,
    },
  },
};

// Handle a new connection from a controller session, performing any necessary setup or initialization for the session.
pub async fn handle_connection(
  session_id: usize,
  session: &mut Session,
  app_data: &AppState,
) -> Result<(), Option<CloseReason>> {
  if let Err(e) = session
    .binary(
      Ready {
        id: session_id as u64,
        streamers: app_data.get_all_streamer_sessions(),
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
  session: &mut Session,
  app_data: &AppState,
  message: impl Buf,
) -> Result<(), Option<CloseReason>> {
  let client_to_server = ClientToServer::decode(message).map_err(|e| {
    tracing::warn!("Failed to decode message from controller: {e}");
    Some(CloseReason {
      code: CloseCode::Unsupported,
      description: Some("Invalid message format".into()),
    })
  })?;

  match client_to_server.command {
    Some(ClientCommand::RequestRtcOffer(command)) => {
      if let Some(mut remote_session) =
        app_data.get_session((Role::Streamer, command.streamer_id as usize).into())
      {
        tracing::debug!(
          "Received RTC offer request for streamer session {}",
          command.streamer_id
        );
        remote_session
          .binary(
            StreamerServerToClient {
              command: Some(StreamerServerCommand::CreateRtcOffer(CreateRtcOffer {
                session: Some(ProtoSession {
                  id: session_id as u64,
                  r#type: SessionType::Controller as i32,
                }),
              })),
            }
            .encode_to_vec(),
          )
          .await
          .map_err(|e| {
            tracing::warn!("Failed to send RTC offer request to streamer session: {e}");
            Some(CloseReason {
              code: CloseCode::Error,
              description: Some("Failed to send RTC offer request".into()),
            })
          })?;

        Ok(())
      } else {
        tracing::warn!(
          "Streamer session with ID {} not found for RTC offer request",
          command.streamer_id
        );
        Ok(())
      }
    }

    Some(ClientCommand::RequestChangeQuality(command)) => {
      if let Some(mut remote_session) =
        app_data.get_session((Role::Streamer, command.streamer_id as usize).into())
      {
        tracing::debug!(
          "Received change quality request for streamer session {} with quality {:?}",
          command.streamer_id,
          Quality::try_from(command.quality)
        );
        remote_session
          .binary(
            StreamerServerToClient {
              command: Some(StreamerServerCommand::ChangeQuality(ChangeQuality {
                quality: command.quality,
              })),
            }
            .encode_to_vec(),
          )
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

    Some(ClientCommand::ListStreamers(_)) => {
      tracing::debug!("Received request for list of streamers");
      session
        .binary(
          ServerToClient {
            command: Some(ServerCommand::ListStreamersResponse(
              ListStreamersResponse {
                streamers: app_data.get_all_streamer_sessions(),
              },
            )),
          }
          .encode_to_vec(),
        )
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

    Some(ClientCommand::ListViewers(command)) => {
      tracing::debug!(
        "Received request for list of viewers for streamer session {:?}",
        command.streamer_id
      );
      session
        .binary(
          ServerToClient {
            command: Some(ServerCommand::ListViewersResponse(
              crate::proto::controller::ListViewersResponse {
                viewers: app_data
                  .get_viewer_sessions_by_streamer(command.streamer_id.map(|id| id as usize)),
              },
            )),
          }
          .encode_to_vec(),
        )
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

    Some(ClientCommand::ListAllViewers(_)) => {
      tracing::debug!("Received request for list of all viewers");
      session
        .binary(
          ServerToClient {
            command: Some(ServerCommand::ListViewersResponse(ListViewersResponse {
              viewers: app_data.get_all_viewer_sessions(),
            })),
          }
          .encode_to_vec(),
        )
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

    Some(ClientCommand::IceCandidate(command)) => {
      if let Some(remote_session_id) = command.session
        && remote_session_id.r#type == SessionType::Streamer as i32
        && let Some(mut remote_session) =
          app_data.get_session((Role::Streamer, remote_session_id.id as usize).into())
      {
        tracing::debug!(
          "Received ICE candidate for streamer session {:?}",
          command.session
        );
        remote_session
          .binary(
            StreamerServerToClient {
              command: Some(StreamerServerCommand::IceCandidate(IceCandidate {
                candidate: command.candidate,
                session: Some(ProtoSession {
                  id: session_id as u64,
                  r#type: SessionType::Controller as i32,
                }),
              })),
            }
            .encode_to_vec(),
          )
          .await
          .map_err(|e| {
            tracing::warn!(
              "Failed to send ICE candidate to streamer {}: {e}",
              remote_session_id.id
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

    Some(ClientCommand::RequestZoom(command)) => {
      if let Some(mut remote_session) =
        app_data.get_session((Role::Streamer, command.streamer_id as usize).into())
      {
        tracing::debug!(
          "Received zoom level change request for streamer session {} with zoom level {}",
          command.streamer_id,
          command.zoom
        );
        remote_session
          .binary(
            StreamerServerToClient {
              command: Some(StreamerServerCommand::ChangeZoom(Zoom {
                zoom: command.zoom,
              })),
            }
            .encode_to_vec(),
          )
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

    Some(ClientCommand::UpdateWatching(command)) => {
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

        if let Some(mut streamer_session) =
          app_data.get_session((Role::Streamer, streamer_id as usize).into())
        {
          streamer_session
            .binary(
              StreamerServerToClient {
                command: Some(StreamerServerCommand::CreateRtcOffer(CreateRtcOffer {
                  session: Some(ProtoSession {
                    id: command.viewer_id,
                    r#type: SessionType::Viewer as i32,
                  }),
                })),
              }
              .encode_to_vec(),
            )
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
        app_data.get_session((Role::Viewer, command.viewer_id as usize).into())
      {
        tracing::debug!(
          "Updated watching status for viewer session {} to not watching any streamer",
          command.viewer_id,
        );

        viewer_session
          .binary(
            ViewerServerToClient {
              command: Some(ViewerServerCommand::DisconnectStreamer(
                DisconnectStreamer {},
              )),
            }
            .encode_to_vec(),
          )
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

    command => {
      tracing::warn!("Received unsupported command from controller: {command:?}");
      Err(Some(CloseReason {
        code: CloseCode::Unsupported,
        description: Some("Unsupported command".into()),
      }))
    }
  }
}
