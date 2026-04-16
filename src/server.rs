use std::{fmt, str::FromStr};

use actix_web::web::Data;
use actix_ws::Session;
use bytestring::ByteString;

use crate::{
  AppState,
  app_state::{Role, SessionId},
  commands::b_command,
  log,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Quality {
  #[default]
  High = 1,
  Medium = 2,
  Low = 3,
}

impl TryFrom<usize> for Quality {
  fn try_from(value: usize) -> Result<Self, Self::Error> {
    match value {
      1 => Ok(Self::High),
      2 => Ok(Self::Medium),
      3 => Ok(Self::Low),
      _ => Err(()),
    }
  }

  type Error = ();
}

impl From<Quality> for usize {
  fn from(value: Quality) -> Self {
    value as usize
  }
}

impl From<Quality> for String {
  fn from(value: Quality) -> Self {
    usize::from(value).to_string()
  }
}

impl fmt::Display for Quality {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", String::from(*self))
  }
}

impl FromStr for Quality {
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Self::try_from(usize::from_str(s).map_err(|_| ())?)
  }

  type Err = ();
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JSONBody {
  pub body: String,
  pub id: usize,
}

impl From<&JSONBody> for String {
  fn from(value: &JSONBody) -> Self {
    format!("{}:#{}", value.id, value.body)
  }
}

impl fmt::Display for JSONBody {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", String::from(self))
  }
}

impl FromStr for JSONBody {
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let mut parts = s.split(":");
    let id = parts.next().ok_or(())?;
    let collected_body = parts.collect::<Vec<_>>().join(":");
    let json_body = collected_body.strip_prefix("#").ok_or(())?;
    Ok(Self {
      body: json_body.to_string(),
      id: usize::from_str(id).map_err(|_| ())?,
    })
  }

  type Err = ();
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Commands {
  A,
  B,
  C { id: Vec<usize> },
  D,
  E { id: usize },
  F(JSONBody),
  G(JSONBody),
  H(JSONBody),
  I { id: usize, quality: Quality },
  J { body: String },
  K { id: usize },
  L { id: usize },
}

impl From<&Commands> for String {
  fn from(value: &Commands) -> Self {
    match value {
      Commands::A => String::from("a"),
      Commands::B => String::from("b"),
      Commands::D => String::from("d"),

      Commands::E { id } => format!("e:{id}"),
      Commands::K { id } => format!("k:{id}"),
      Commands::L { id } => format!("l:{id}"),

      Commands::F(body) => format!("f:{body}"),
      Commands::G(body) => format!("g:{body}"),
      Commands::H(body) => format!("h:{body}"),

      Commands::I { id, quality } => format!("i:{id}:{quality}"),

      Commands::J { body } => format!("j:#{body}"),

      Commands::C { id } => format!(
        "c:%{}",
        id.iter()
          .map(ToString::to_string)
          .collect::<Vec<_>>()
          .join(",")
      ),
    }
  }
}

impl fmt::Display for Commands {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", String::from(self))
  }
}

impl FromStr for Commands {
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let mut chars = s.chars();
    let cmd = chars.next().ok_or(())?;
    let rest = chars.skip(1).collect::<String>();

    match cmd {
      'a' => Ok(Self::A),
      'b' => Ok(Self::B),
      'd' => Ok(Self::D),

      'e' | 'k' | 'l' => {
        let id = usize::from_str(&rest).map_err(|_| ())?;
        if cmd == 'e' {
          Ok(Self::E { id })
        } else if cmd == 'l' {
          Ok(Self::L { id })
        } else {
          Ok(Self::K { id })
        }
      }

      'f' | 'g' | 'h' => match cmd {
        'f' => Ok(Self::F(JSONBody::from_str(&rest)?)),
        'g' => Ok(Self::G(JSONBody::from_str(&rest)?)),
        _ => Ok(Self::H(JSONBody::from_str(&rest)?)),
      },

      'i' => {
        let mut parts = rest.split(":");
        let id_part = parts.next().ok_or(())?;
        let quality_part = parts.next().ok_or(())?;

        let id = usize::from_str(id_part).map_err(|_| ())?;
        let quality = Quality::from_str(quality_part)?;

        Ok(Self::I { id, quality })
      }

      'j' => Ok(Self::J {
        body: rest.strip_prefix('#').ok_or(())?.to_string(),
      }),

      'c' => {
        let ids = rest
          .strip_prefix('%')
          .ok_or(())?
          .split(',')
          .map(usize::from_str)
          .collect::<Result<Vec<_>, _>>()
          .map_err(|_| ())?;

        Ok(Self::C { id: ids })
      }

      _ => Err(()),
    }
  }

  type Err = ();
}

pub async fn message_handler(
  app_state: Data<AppState>,
  session_id: SessionId,
  session: &mut Session,
  content: ByteString,
) {
  let Ok(command) = Commands::from_str(&content) else {
    log(&format!("Invalid command: {content}"), None);
    return;
  };

  match command {
    Commands::B => {
      b_command(&app_state, session.clone()).await;
    }

    Commands::D => {
      if let Some(viewer_index) = session_id.as_viewer()
        && let Some(streamer_session) = app_state.get_streamer_session_from_viewer(viewer_index)
      {
        if let Some(mut streamer_session) = streamer_session {
          let _ = streamer_session
            .text(Commands::E { id: viewer_index }.to_string())
            .await;
        } else {
          log(
            "[Warn] Viewer asked for streamer but is not watching anyone",
            None,
          );
        }
      } else {
        log(
          "[Warn] Non-viewer or a invalid viewer asked for streamer",
          None,
        );
      }
    }
    Commands::F(data) => {
      if let Some(mut remote_session) = app_state.get_session((Role::Viewer, data.id).into()) {
        let _ = remote_session
          .text(
            Commands::F(JSONBody {
              body: data.body,
              id: *session_id,
            })
            .to_string(),
          )
          .await; // sends F as is to viewer
      } else {
        log("[Warn] Streamer offered to no one", None);
      }
    }

    Commands::G(data) => {
      if let Some(mut remote_session) = app_state.get_session((Role::Viewer, data.id).into()) {
        let _ = remote_session
          .text(
            Commands::G(JSONBody {
              body: data.body,
              id: *session_id,
            })
            .to_string(),
          )
          .await; // sends G as is to Viewer
      } else {
        log("[Warn] Streamer sent candidate to no one", None);
      }
    }

    Commands::H(data) => {
      if let Some(mut remote_session) = app_state.get_session((Role::Streamer, data.id).into()) {
        let _ = remote_session
          .text(
            Commands::H(JSONBody {
              body: data.body,
              id: *session_id,
            })
            .to_string(),
          )
          .await; // sends H as is to Streamer
      } else {
        log("[Warn] Streamer sent candidate to no one", None);
      }
    }

    Commands::I { id, quality } => {
      if let Some(mut remote_session) = app_state.get_session((Role::Streamer, id).into()) {
        let _ = remote_session
          .text(Commands::I { id, quality }.to_string())
          .await; // sends I as is to Streamer
      } else {
        log("[Warn] Controller sent quality request to no one", None);
      }
    }

    Commands::J { body } => {
      for mut remote_session in app_state.get_all_controllers_and_viewers_sessions() {
        let _ = remote_session
          .text(Commands::J { body: body.clone() }.to_string())
          .await; // Broadcast J for everyone except streamers
      }
    }

    _ => {}
  };
}
