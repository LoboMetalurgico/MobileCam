//! Application state management for the MobileCam server, including data structures for viewers, streamers, and controllers.

use actix_ws::Session;

use crate::sparse_set::SyncSparseSet;

/// Viewer data structure, containing the session ID and an optional field for the streamer they are watching.
pub struct ViewerData {
  /// The session associated with the viewer.
  pub session: Session,
  /// An optional field indicating the streamer that the viewer is currently watching.
  pub watching: Option<usize>,
}

/// Streamer data structure, containing the session ID.
pub struct StreamerData {
  /// The session associated with the streamer.
  pub session: Session,
}

/// Controller data structure, containing the session ID.
pub struct ControllerData {
  /// The session associated with the controller.
  pub session: Session,
}

/// Represents the role of a client in the MobileCam system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, derive_more::FromStr)]
#[from_str(error(InvalidRoleError))]
pub enum Role {
  /// The viewer role, representing clients that watch the stream.
  Viewer,
  /// The streamer role, representing clients that broadcast the stream.
  Streamer,
  /// The controller role, representing clients that can control the stream.
  Controller,
}

/// Custom error type for invalid role errors.
#[derive(Debug, Clone, derive_more::Display, derive_more::Error, derive_more::From)]
#[from(derive_more::FromStrError)]
#[display("Invalid role provided")]
pub struct InvalidRoleError;

/// A mixed index structure that combines a session index with a role-specific index for efficient access to session data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, derive_more::From, derive_more::Into, derive_more::Deref)]
#[into]
pub struct SessionId(
  #[into]
  Role,

  #[into]
  #[deref]
  usize
);

impl SessionId {
  /// Retrieves the index if the session is a viewer, returning `None` if the session is not.
  pub fn as_viewer(self) -> Option<usize> {
    if self.0 == Role::Viewer {
      Some(self.1)
    } else {
      None
    }
  }
}

impl AsRef<Role> for SessionId {
  fn as_ref(&self) -> &Role {
    &self.0
  }
}

impl AsRef<usize> for SessionId {
  fn as_ref(&self) -> &usize {
    &self.1
  }
}

impl PartialEq<Role> for SessionId {
  fn eq(&self, other: &Role) -> bool {
    self.0 == *other
  }
}

impl PartialEq<usize> for SessionId {
  fn eq(&self, other: &usize) -> bool {
    self.1 == *other
  }
}

/// The main application state, containing sparse sets for sessions, streamers, controllers, and viewers.
pub struct AppState {
  /// A sparse set for managing streamer data.
  streamers: SyncSparseSet<StreamerData>,
  /// A sparse set for managing controller data.
  controllers: SyncSparseSet<ControllerData>,
  /// A sparse set for managing viewer data.
  viewers: SyncSparseSet<ViewerData>,
}

impl AppState {
  pub fn new() -> Self {
    Self {
      streamers: SyncSparseSet::with_capacity(1),
      controllers: SyncSparseSet::with_capacity(1),
      viewers: SyncSparseSet::with_capacity(1),
    }
  }

  pub fn insert(&self, role: Role, session: Session) -> SessionId {
    (role, match role {
      Role::Streamer => self.streamers.insert(StreamerData { session }),
      Role::Controller => self.controllers.insert(ControllerData { session }),
      Role::Viewer => self.viewers.insert(ViewerData { session, watching: None }),
    }).into()
  }

  /// Removes a session from the appropriate sparse set based on its role.
  pub fn remove_session(&self, session_id: SessionId) -> Option<Session> {
    match Role::from(session_id) {
      Role::Streamer => { self.streamers.remove(*session_id).map(|data| data.session) }
      Role::Controller => { self.controllers.remove(*session_id).map(|data| data.session) }
      Role::Viewer => { self.viewers.remove(*session_id).map(|data| data.session) }
    }
  }

  /// Retrieves a session from the appropriate sparse set based on its role.
  pub fn get_session(&self, session_id: SessionId) -> Option<Session> {
    match Role::from(session_id) {
      Role::Streamer => self.streamers.view(*session_id, |v| v.session.clone()),
      Role::Controller => self.controllers.view(*session_id, |v| v.session.clone()),
      Role::Viewer => self.viewers.view(*session_id, |v| v.session.clone()),
    }
  }

  /// Retrieves the streamer that a viewer is currently watching, if any.
  /// 
  /// The first `Option` indicates if there is a valid viewer index, while the second `Option` indicates if the viewer is watching a valid streamer.
  pub fn get_streamer_session_from_viewer(&self, viewer_index: usize) -> Option<Option<Session>> {
    self.viewers.view(viewer_index, |v| v.watching).map(|streamer_index| streamer_index.and_then(|i| self.streamers.view(i, |s| s.session.clone())))
  }

  /// Retrieves all sessions for both controllers and viewers.
  pub fn get_all_controllers_and_viewers_sessions(&self) -> impl Iterator<Item = Session> {
    self.controllers.map(|(_, v)| v.session.clone()).into_iter().chain(
      self.viewers.map(|(_, v)| v.session.clone())
    )
  }

  pub fn get_all_streamers_id(&self) -> Vec<usize> {
    self.streamers.map(|(i, _)| i)
  }
}
