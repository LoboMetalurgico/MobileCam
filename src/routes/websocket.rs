use std::{str::FromStr, time::Duration};

use actix_web::{
  Error, HttpRequest, HttpResponse, get, rt,
  web::{self, Data},
};
use actix_ws::{AggregatedMessage, CloseReason, Session};
use bytestring::ByteString;
use serde::Deserialize;
use tokio::time::sleep;
use tracing::Instrument;

use crate::{
  CONN_TIMEOUT, MSG_TIMEOUT,
  app_state::{AppState, Role, SessionId},
  server::{Commands, message_handler},
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

#[derive(Debug, Deserialize)]
struct WebSocketQuery {
  role: Option<String>,
}

async fn handle_message(
  app_state: Data<AppState>,
  session_id: SessionId,
  session: &mut Session,
  message: AggregatedMessage,
) -> Option<Option<CloseReason>> {
  let content = match message {
    AggregatedMessage::Close(reason) => {
      if session_id == Role::Streamer {
        tracing::debug!("Notifying viewers and controllers about streamer disconnection");
        for mut conn in app_state.get_all_controllers_and_viewers_sessions() {
          let _ = conn.text(Commands::K { id: *session_id }.to_string()).await;
        }
      }

      return Some(reason);
    }

    AggregatedMessage::Binary(data) => match ByteString::try_from(data)
      .inspect(|_| tracing::debug!("Received text content on a binary message, treating as text"))
    {
      Ok(v) => v,
      Err(e) => {
        tracing::warn!("Unexpected binary message: {e}");
        return None;
      }
    },

    AggregatedMessage::Text(txt) => txt,

    _ => return None,
  };

  message_handler(app_state, session_id, session, content).await;

  None
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

  let session_id = app_state.insert(role, session.clone());

  tracing::info!("New session: {session_id}");

  match role {
    Role::Streamer => {
      let _ = session
        .text(Commands::L { id: *session_id }.to_string())
        .await; // sends L as is to Streamer, letting them know their session ID

      for mut init_session in app_state.get_all_controllers_and_viewers_sessions() {
        let _ = init_session.text("a").await;
      }
    }
    _ => {}
  };

  rt::spawn(async move {
    let mut message_stream = message_stream.aggregate_continuations();
    let mut timeout_task = rt::spawn(timeout(session.clone()).instrument(tracing::Span::current()));

    let close_reason = loop {
      let message_timeout = sleep(Duration::from_secs(MSG_TIMEOUT));

      tokio::select! {
        Some(Ok(message)) =  message_stream.recv() => {
          timeout_task.abort();
          if let Some(v) = handle_message(app_state.clone(), session_id, &mut session, message).await {
            break v;
          } else {
            tracing::trace!("Creating new timeout task");
            timeout_task = rt::spawn(timeout(session.clone()).instrument(tracing::Span::current()));
          }
        }

        _ = message_timeout => {
          tracing::trace!("Message timeout reached, sending ping to check connection health");
          if let Err(e) = session.ping(b"").await {
            tracing::debug!("Failed to send ping: {e}");
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
