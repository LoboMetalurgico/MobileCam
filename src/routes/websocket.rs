use std::{str::FromStr, time::Duration};

use actix_web::{
  Error, HttpRequest, HttpResponse, get, rt,
  web::{self, Data},
};
use actix_ws::{AggregatedMessage, CloseReason, Session};
use bytestring::ByteString;
use tokio::time::sleep;

use crate::{
  CONN_TIMEOUT, MSG_TIMEOUT, app_state::AppState, commands::b_command, log, server::{Commands, Roles, message_handler}
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

async fn handle_msg(
  msg: AggregatedMessage,
  app_state: Data<AppState>,
  session_id: u8,
  session: &Session,
  role: Roles,
) -> Option<Option<CloseReason>> {
  let content = match msg {
    AggregatedMessage::Close(reason) => {
      if role.is_streamer() {
        let consumers = app_state.get_conns(|u| u.role.is_viewer() || u.role.is_controller());
        for mut conn in consumers {
          let _ = conn.text(Commands::K { id: session_id }.to_string());
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

  message_handler(app_state, (session_id, session), role, content).await;

  None
}

#[get("/ws")]
async fn incoming_socket(
  req: HttpRequest,
  stream: web::Payload,
  app_state: Data<AppState>,
) -> Result<HttpResponse, Error> {
  let role = match req
    .headers()
    .get("stream-role")
    .and_then(|v| v.to_str().ok())
  {
    Some("streamer") => Roles::Streamer,
    Some("viewer") => {
      let Some(viewing_id) = req
        .headers()
        .get("watch-id")
        .and_then(|v| v.to_str().ok().and_then(|s| u8::from_str(s).ok()))
      else {
        return Ok(
          HttpResponse::BadRequest().body("Watch-id header value is not a valid u8 number!"),
        );
      };
      Roles::Viewer(viewing_id)
    }
    Some("controller") => Roles::Controller,
    _ => {
      return Ok(
        HttpResponse::BadRequest().body("Missing or invalid stream-role header in request!"),
      );
    }
  };

  let Some(session_id) = app_state.recycle() else {
    log("[Warn] Too many sockets, Memory full!", None);
    return Ok(HttpResponse::InsufficientStorage().body("Too many sockets already connected!"));
  };

  let (res, mut session, msg_stream) = actix_ws::handle(&req, stream)?;

  app_state.register(session_id, session.clone(), role);

  match role {
    Roles::Streamer => {
      for mut init_session in app_state
        .get_conns(|user_data| user_data.role.is_controller() || user_data.role.is_viewer())
      {
        let _ = init_session.text("a").await;
      }
    }
    Roles::Viewer(data) => {
      if let Some(mut conn) = app_state.get_connection(data) {
        let _ = conn.text(format!("e:{session_id}")).await;
      }
    }
    Roles::Controller => {
      let _ = b_command(&app_state, session.clone());
    }
  }

  rt::spawn(async move {
    let mut timeout_task = create_timeout_task(session.clone());
    let mut msg_stream = msg_stream.aggregate_continuations();

    let close_reason = loop {
      let msg_timeout = sleep(Duration::from_secs(MSG_TIMEOUT));

      tokio::select! {
        Some(Ok(msg)) = msg_stream.recv() => {
          timeout_task.abort();
          if let Some(v) = handle_msg(msg, app_state.clone(), session_id, &session, role).await {
            break v;
          } else {
            timeout_task = create_timeout_task(session.clone());
          }
        }

        _ = msg_timeout => {
          let _ = session.ping(b"").await;
        }
      }
    };

    let _ = session.close(close_reason).await;
    app_state.discard(session_id);
  });

  Ok(res)
}
