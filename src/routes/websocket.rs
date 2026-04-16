use std::{str::FromStr, time::Duration};

use actix_web::{
  Error, HttpRequest, HttpResponse, get, rt,
  web::{self, Data},
};
use actix_ws::{AggregatedMessage, CloseReason, Session};
use bytestring::ByteString;
use serde::Deserialize;
use tokio::time::sleep;

use crate::{
  CONN_TIMEOUT, MSG_TIMEOUT, app_state::{AppState, Role, SessionId}, log, server::{Commands, message_handler}
};

fn create_timeout_task(session: Session) -> rt::task::JoinHandle<()> {
  rt::spawn(async move {
    sleep(Duration::from_secs(CONN_TIMEOUT)).await;
    let _ = session
      .close(Some(CloseReason {
        code: 4000.into(),
        description: Some("Connection Timeout".to_string()),
      }))
      .await;
  })
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
        for mut conn in app_state.get_all_controllers_and_viewers_sessions() {
          let _ = conn.text(Commands::K { id: *session_id }.to_string()).await;
        }
      }

      return Some(reason)
    },

    AggregatedMessage::Binary(data) => match ByteString::try_from(data) {
      Ok(v) => v,
      Err(e) => {
        log(&format!("Client is weird: {e}"), None);
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
async fn incoming_socket(
  req: HttpRequest,
  stream: web::Payload,
  app_state: Data<AppState>,
  query: web::Query<WebSocketQuery>,
) -> Result<HttpResponse, Error> {
  let Some(role) = query.role.as_deref().and_then(|s| Role::from_str(s).ok()) else {
    log("WebSocket connection requires a valid role query parameter!", None);
    return Ok(
      HttpResponse::BadRequest().body("Missing or invalid role query parameter in request!"),
    );
  };

  let (res, mut session, msg_stream) = actix_ws::handle(&req, stream)?;

  let session_id = app_state.insert(role, session.clone());

  match role {
    Role::Streamer => {
      log(&format!("Streamer connected with session ID {session_id:?}"), None);

      let _ = session.text(Commands::L { id: *session_id }.to_string()).await; // sends L as is to Streamer, letting them know their session ID

      for mut init_session in app_state.get_all_controllers_and_viewers_sessions() {
        let _ = init_session.text("a").await;
      }
    }
    Role::Viewer => {
      log(&format!("Viewer connected with session ID {session_id:?}"), None);
    }
    Role::Controller => {
      log(&format!("Controller connected with session ID {session_id:?}"), None);
    }
  };

  rt::spawn(async move {
    let mut timeout_task = create_timeout_task(session.clone());
    let mut msg_stream = msg_stream.aggregate_continuations();

    let close_reason = loop {
      let msg_timeout = sleep(Duration::from_secs(MSG_TIMEOUT));

      tokio::select! {
        Some(Ok(msg)) = msg_stream.recv() => {
          timeout_task.abort();
          if let Some(v) = handle_message(app_state.clone(), session_id, &mut session, msg).await {
            break v;
          } else {
            timeout_task = create_timeout_task(session.clone());
          }
        }

        _ = msg_timeout => {
          if session.ping(b"").await.is_err() {
            break Some(CloseReason {
              code: 4001.into(),
              description: Some("Message Timeout".to_string()),
            });
          }
        }
      }
    };

    let _ = session.close(close_reason).await;
    app_state.remove_session(session_id);
  });

  Ok(res)
}
