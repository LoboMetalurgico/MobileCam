use std::{
  fmt,
  str::FromStr,
};

use actix_web::web::Data;
use actix_ws::Session;
use bytestring::ByteString;

use crate::{AppState, commands::b_command, log};

#[derive(Debug, Clone, PartialEq, Copy, Eq)]
pub enum Roles {
  Streamer,
  Viewer(u8),
  Controller,
}

impl Roles {
  pub fn is_viewer(&self) -> bool {
    matches!(self, Roles::Viewer(_))
  }
  pub fn is_controller(&self) -> bool {
    matches!(self, Roles::Controller)
  }
  pub fn is_streamer(&self) -> bool {
    matches!(self, Roles::Streamer)
  }
}

#[derive(Clone)]
pub struct UserData {
  pub session: Session,
  pub role: Roles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Quality {
  High = 1,
  Medium = 2,
  Low = 3,
}

impl TryFrom<u8> for Quality {
  fn try_from(value: u8) -> Result<Self, Self::Error> {
    match value {
      1 => Ok(Self::High),
      2 => Ok(Self::Medium),
      3 => Ok(Self::Low),
      _ => Err(()),
    }
  }

  type Error = ();
}

impl From<Quality> for u8 {
  fn from(value: Quality) -> Self {
    value as u8
  }
}

impl From<Quality> for String {
  fn from(value: Quality) -> Self {
    u8::from(value).to_string()
  }
}

impl fmt::Display for Quality {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", String::from(*self))
  }
}

impl FromStr for Quality {
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Self::try_from(u8::from_str(s).map_err(|_| ())?)
  }

  type Err = ();
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JSONBody {
  pub body: String,
  pub id: u8,
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
      id: u8::from_str(id).map_err(|_| ())?,
    })
  }

  type Err = ();
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Commands {
  A,
  B,
  C { id: Vec<u8> },
  D,
  E { id: u8 },
  F(JSONBody),
  G(JSONBody),
  H(JSONBody),
  I { id: u8, quality: Quality },
  J { body: String },
  K { id: u8 },
}

impl From<&Commands> for String {
  fn from(value: &Commands) -> Self {
    match value {
      Commands::A => String::from("a"),
      Commands::B => String::from("b"),
      Commands::D => String::from("d"),

      Commands::E { id } => format!("e:{id}"),
      Commands::K { id } => format!("e:{id}"),

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

      'e' | 'k' => {
        let id = u8::from_str(&rest).map_err(|_| ())?;
        if cmd == 'e' {
          Ok(Self::E { id })
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

        let id = u8::from_str(id_part).map_err(|_| ())?;
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
          .map(u8::from_str)
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
  (session_id, session): (u8, &Session),
  role: Roles,
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
      if let Roles::Viewer(id) = role
        && let Some(mut connection) = app_state.get_connection(id)
      {
        let _ = connection
          .text(Commands::E { id: session_id }.to_string())
          .await;
      } else {
        log("[Warn] Non viewer asked for streamer", None);
      }
    }
    Commands::F(data) => {

      if let Some(mut connection) = app_state.get_connection(data.id) {
        let _ = connection.text(Commands::F(JSONBody { body: data.body, id: session_id }).to_string()).await; // sends F as is to viewer
      } else {
        log("[Warn] Streamer offered to no one", None);
      }
    }

    Commands::G(data) => {
      if let Some(mut connection) = app_state.get_connection(data.id) {
        let _ = connection.text(Commands::G(JSONBody { body: data.body, id: session_id }).to_string()).await; // sends G as is to Viewer
      } else {
        log("[Warn] Streamer sent candidate to no one", None);
      }
    }

    Commands::H(data) => {
      if let Some(mut connection) = app_state.get_connection(data.id) {
        let _ = connection.text(Commands::H(JSONBody { body: data.body, id: session_id }).to_string()).await; // sends H as is to Streamer
      } else {
        log("[Warn] Streamer sent candidate to no one", None);
      }
    }

    Commands::I { id, quality } => {
      if let Some(mut connection) = app_state.get_connection(id) {
        let _ = connection.text(Commands::I { id, quality }.to_string()).await; // sends I as is to Streamer
      } else {
        log("[Warn] Controller sent quality request to no one", None);
      } 
    }

    Commands::J { body } => {
      let connection = app_state.get_conns(|data| { data.role.is_viewer() || data.role.is_controller() });
      for mut conn in connection {
        let _ = conn.text(Commands::J { body: body.clone() }.to_string()).await; // Broadcast J for everyone except streamers
      } 
    }

    _ => {}
  };
}
