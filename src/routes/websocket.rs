use std::{str::FromStr, time::Duration};

use actix_web::{
  Error, HttpRequest, HttpResponse, get, rt,
  web::{self, Data},
};
use actix_ws::{AggregatedMessage, CloseCode, CloseReason, Session};
use serde::Deserialize;
use tokio::time::sleep;
use tracing::Instrument;

use crate::{
  CONN_TIMEOUT, MSG_TIMEOUT,
  app_state::{AppState, Role, SessionId},
  proto::controller::ControllerSessionRef,
  server::{controller, streamer, viewer},
};

async fn timeout(session: Session) {
  sleep(Duration::from_secs(CONN_TIMEOUT)).await;

  tracing::info!("Connection timeout");
  let _ = session
    .close(Some(CloseReason {
      code: 4000.into(),
      description: Some("Connection Timeout".to_string()),
    }))
    .await;
}

async fn handle_message(
  app_state: Data<AppState>,
  session_id: SessionId,
  session: &mut Session,
  message: AggregatedMessage,
) -> Result<(), Option<CloseReason>> {
  match message {
    AggregatedMessage::Close(reason) => Err(reason),

    AggregatedMessage::Binary(data) => match Role::from(session_id) {
      Role::Streamer => streamer::handle_message(*session_id, &app_state, data).await,
      Role::Controller => {
        controller::handle_message(
          *session_id,
          ControllerSessionRef::from(session),
          &app_state,
          data,
        )
        .await
      }
      Role::Viewer => viewer::handle_message(*session_id, &app_state, data).await,
    },

    AggregatedMessage::Text(_) => {
      tracing::warn!("Text messages are not supported");
      Err(Some(CloseReason {
        code: CloseCode::Unsupported,
        description: Some("Text messages are not supported.".into()),
      }))
    }

    _ => Ok(()),
  }
}

#[derive(Debug, Deserialize)]
struct WebSocketQuery {
  role: Option<String>,
  name: Option<String>,
}

#[get("/ws")]
#[tracing::instrument(skip_all, name = "websocket_route")]
async fn incoming_socket(
  req: HttpRequest,
  stream: web::Payload,
  app_state: Data<AppState>,
  query: web::Query<WebSocketQuery>,
) -> Result<HttpResponse, Error> {
  let Some(role) = query.role.as_deref().and_then(|s| Role::from_str(s).ok()) else {
    tracing::warn!("WebSocket connection requires a valid role query parameter!");
    return Ok(
      HttpResponse::BadRequest().body("Missing or invalid role query parameter in request!"),
    );
  };

  let (res, mut session, message_stream) = actix_ws::handle(&req, stream)?;

  let session_id = match role {
    Role::Controller => app_state.add_controller(session.clone()),
    Role::Streamer => app_state.add_streamer(session.clone(), query.name.as_deref()),
    Role::Viewer => app_state.add_viewer(session.clone(), query.name.as_deref()),
  };

  tracing::info!("New session: {session_id}");

  let early_close = match role {
    Role::Controller => {
      controller::handle_connection(
        *session_id,
        ControllerSessionRef::from(&mut session),
        &app_state,
      )
      .await
    }
    Role::Streamer => streamer::handle_connection(*session_id, &app_state).await,
    Role::Viewer => viewer::handle_connection(*session_id, &app_state).await,
  };

  if let Err(reason) = early_close {
    let _ = session.close(reason).await;
    app_state.remove_session(session_id);
    return Ok(res);
  }

  rt::spawn(async move {
    let mut message_stream = message_stream.aggregate_continuations();
    let mut timeout_task = rt::spawn(timeout(session.clone()).instrument(tracing::Span::current()));

    let close_reason = loop {
      let message_timeout = sleep(Duration::from_secs(MSG_TIMEOUT));

      tokio::select! {
        Some(Ok(message)) =  message_stream.recv() => {
          timeout_task.abort();
          if let Err(v) = handle_message(app_state.clone(), session_id, &mut session, message).await {
            break v;
          } else {
            tracing::trace!("Creating new timeout task");
            timeout_task = rt::spawn(timeout(session.clone()).instrument(tracing::Span::current()));
          }
        }

        _ = message_timeout => {
          tracing::trace!("Message timeout reached, sending ping to check connection health");
          if let Err(e) = session.ping(b"").await {
            tracing::warn!("Failed to send ping: {e}");
            timeout_task.abort();
            break Some(CloseReason {
              code: 4001.into(),
              description: Some("Message Timeout".to_string()),
            });
          }
        }
      }
    };

    let log_close_reason = close_reason.as_ref().map(|v| v.code);
    let _ = session.close(close_reason).await;
    app_state.remove_session(session_id);
    tracing::info!("Session closed with reason: {log_close_reason:?}");
  }.instrument(tracing::info_span!(parent: None, "websocket_handler", session_id = *session_id)));

  Ok(res)
}
