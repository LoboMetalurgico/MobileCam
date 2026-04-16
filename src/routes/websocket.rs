use std::time::Duration;

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
  CONN_TIMEOUT, MSG_TIMEOUT, app_state::{AppState}, server::{Commands, Roles, message_handler}
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
  message: AggregatedMessage,
  app_state: Data<AppState>,
  session_id: u8,
  session: &Session,
  role: Roles,
) -> Option<Option<CloseReason>> {
  tracing::trace!("Received message: {message:?}");
  let content = match message {
    AggregatedMessage::Close(reason) => {
      tracing::info!("Connection closed with reason: {reason:?}");

      if role.is_streamer() {
        tracing::debug!("Notifying viewers and controllers about streamer disconnection for session {session_id}");
        let consumers = app_state.get_conns(|u| u.role.is_viewer() || u.role.is_controller());
        for mut conn in consumers {
          let _ = conn.text(Commands::K { id: session_id }.to_string()).await;
        }
      }

      return Some(reason)
    },

    AggregatedMessage::Binary(data) => match ByteString::try_from(data).inspect(|_| tracing::debug!("Received text content on a binary message, treating as text")) {
      Ok(v) => v,
      Err(e) => {
        tracing::warn!("Unexpected binary message from session {session_id}: {e}");
        return None;
      }
    },

    AggregatedMessage::Text(txt) => txt,

    _ => return None,
  };

  message_handler(app_state, (session_id, session), role, content).await;

  None
}
  

#[derive(Debug, Deserialize)]
struct WebSocketQuery {
  role: Option<String>,
  watch_id: Option<u8>,
}

#[get("/ws")]
#[tracing::instrument(skip_all, name = "websocket_route")]
async fn incoming_socket(
  req: HttpRequest,
  stream: web::Payload,
  app_state: Data<AppState>,
  query: web::Query<WebSocketQuery>,
) -> Result<HttpResponse, Error> {
  let role = match query.role.as_deref()
  {
    Some("streamer") => Roles::Streamer,
    Some("viewer") => {
      let Some(viewing_id) = query.watch_id
      else {
        tracing::warn!("Viewer role requires watch_id query parameter set");
        return Ok(
          HttpResponse::BadRequest().body("watch_id value isn't provided for viewer role!"),
        );
      };
      Roles::Viewer(viewing_id)
    }
    Some("controller") => Roles::Controller,
    _ => {
      tracing::warn!("Invalid or missing role query parameter in request");
      return Ok(
        HttpResponse::BadRequest().body("Missing or invalid role query parameter in request!"),
      );
    }
  };

  let Some(session_id) = app_state.recycle() else {
    tracing::warn!("No available session IDs to assign for new connection, rejecting connection");
    return Ok(HttpResponse::InsufficientStorage().body("Too many sockets already connected!"));
  };

  let (res, mut session, message_stream) = actix_ws::handle(&req, stream)?;

  app_state.register(session_id, session.clone(), role);

  match role {
    Roles::Streamer => {
      tracing::info!("Streamer connected with session ID {session_id}");

      let _ = session.text(Commands::L { id: session_id }.to_string()).await; // sends L as is to Streamer, letting them know their session ID

      for mut init_session in app_state
        .get_conns(|user_data| user_data.role.is_controller() || user_data.role.is_viewer())
      {
        let _ = init_session.text("a").await;
      }
    }
    Roles::Viewer(data) => {
      tracing::info!("Viewer connected with session ID {session_id}, watching {data}");

      if let Some(mut conn) = app_state.get_connection(data) {
        let _ = conn.text(format!("e:{session_id}")).await;
      }
    }
    Roles::Controller => {
      tracing::info!("Controller connected with session ID {session_id}");
    }
  }

  rt::spawn(async move {
    let mut message_stream = message_stream.aggregate_continuations();
      let mut timeout_task = rt::spawn(timeout(session.clone()).instrument(tracing::Span::current()));

      let close_reason = loop {
        let  message_timeout = sleep(Duration::from_secs(MSG_TIMEOUT));

        tokio::select! {
          Some(Ok(message)) =  message_stream.recv() => {
            timeout_task.abort();
            if let Some(v) = handle_message(message, app_state.clone(), session_id, &session, role).await {
              tracing::debug!("Closing connection with reason: {v:?}");
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
              break Some(CloseReason {
                code: 4001.into(),
                description: Some("Message Timeout".to_string()),
              });
            }
          }
        }
      };

      let _ = session.close(close_reason).await;
      app_state.discard(session_id);
      tracing::info!("Session disconnected and cleaned up");
    }.instrument(tracing::info_span!(parent: None, "websocket_handler", session_id = session_id, role = ?role)));

  Ok(res)
}
