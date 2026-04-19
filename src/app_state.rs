//! Application state management for the MobileCam server, including data structures for viewers, streamers, and controllers.

use actix_ws::Session;

use crate::{
  proto::{
    common::VideoTransform as ProtoVideoTransform,
    controller::{
      ControllerSession, StreamerSession as ProtoStreamerSession,
      ViewerSession as ProtoViewerSession,
    },
    streamer::StreamerSession,
    viewer::ViewerSession,
  },
  sparse_set::SyncSparseSet,
};

/// Video transform data structure, containing rotation, zoom, and position information for the video stream.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VideoTransform {
  /// The rotation of the video stream in degrees, where 0 is the default orientation, and positive values indicate clockwise rotation.
  pub rotation: i16,
  /// The zoom level of the video stream, where 1.0 is the default zoom level, and values greater than 1.0 indicate zooming in, while values less than 1.0 indicate zooming out.
  pub zoom: f32,
  /// The x-coordinate of the video stream's position.
  pub x: f32,
  /// The y-coordinate of the video stream's position.
  pub y: f32,
}

impl From<VideoTransform> for ProtoVideoTransform {
  fn from(value: VideoTransform) -> Self {
    Self {
      rotation: value.rotation as i32,
      zoom: value.zoom,
      x: value.x,
      y: value.y,
    }
  }
}

impl From<ProtoVideoTransform> for VideoTransform {
  fn from(value: ProtoVideoTransform) -> Self {
    Self {
      rotation: value.rotation as i16,
      zoom: value.zoom,
      x: value.x,
      y: value.y,
    }
  }
}

/// Viewer data structure, containing the session ID and an optional field for the streamer they are watching.
pub struct ViewerData {
  /// The session associated with the viewer.
  pub session: Session,
  /// An optional field indicating the streamer that the viewer is currently watching.
  pub watching: Option<usize>,
  /// An optional field for the name of the viewer, which can be used for display purposes.
  pub name: Option<String>,
}

impl From<(usize, &ViewerData)> for ProtoViewerSession {
  fn from((session_id, viewer_data): (usize, &ViewerData)) -> Self {
    Self {
      id: session_id as u64,
      streamer_id: viewer_data.watching.map(|i| i as u64),
      name: viewer_data.name.clone(),
    }
  }
}

/// Streamer data structure, containing the session ID.
pub struct StreamerData {
  /// The session associated with the streamer.
  pub session: Session,
  /// An optional field for the name of the streamer, which can be used for display purposes.
  pub name: Option<String>,
  /// An optional field for the video transform of the streamer's video stream.
  pub video_transform: Option<VideoTransform>,
  /// An optional field for the battery level of the streamer, which can be used for display purposes.
  pub battery_level: Option<u8>,
}

impl From<(usize, &StreamerData)> for ProtoStreamerSession {
  fn from((session_id, streamer_data): (usize, &StreamerData)) -> Self {
    Self {
      id: session_id as u64,
      name: streamer_data.name.clone(),
      zoom: streamer_data.video_transform.as_ref().map(|t| t.zoom),
      battery_level: streamer_data.battery_level.as_ref().map(|b| *b as u32),
    }
  }
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
#[derive(
  Debug,
  Clone,
  Copy,
  PartialEq,
  Eq,
  derive_more::From,
  derive_more::Into,
  derive_more::Deref,
  derive_more::Display,
)]
#[into]
#[display("{_0:?}({_1})")]
pub struct SessionId(
  #[into] Role,
  #[into]
  #[deref]
  usize,
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

impl PartialEq<SessionId> for Role {
  fn eq(&self, other: &SessionId) -> bool {
    *self == other.0
  }
}

impl PartialEq<usize> for SessionId {
  fn eq(&self, other: &usize) -> bool {
    self.1 == *other
  }
}

impl PartialEq<SessionId> for usize {
  fn eq(&self, other: &SessionId) -> bool {
    *self == other.1
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
  /// Creates a new instance of `AppState` with initialized sparse sets for streamers, controllers, and viewers.
  pub fn new() -> Self {
    Self {
      streamers: SyncSparseSet::with_capacity(1),
      controllers: SyncSparseSet::with_capacity(1),
      viewers: SyncSparseSet::with_capacity(1),
    }
  }

  /// Add a new controller session into the [`AppState`], returning a [`SessionId`] that uniquely identifies the session and its role.
  pub fn add_controller(&self, session: Session) -> SessionId {
    let index = self.controllers.insert(ControllerData { session });
    SessionId(Role::Controller, index)
  }

  /// Add a new streamer session into the [`AppState`], returning a [`SessionId`] that uniquely identifies the session and its role.
  pub fn add_streamer(&self, session: Session, name: Option<&str>) -> SessionId {
    let index = self.streamers.insert(StreamerData {
      session,
      name: name.map(|s| s.into()),
      video_transform: None,
      battery_level: None,
    });
    SessionId(Role::Streamer, index)
  }

  /// Add a new viewer session into the [`AppState`], returning a [`SessionId`] that uniquely identifies the session and its role.
  pub fn add_viewer(&self, session: Session, name: Option<&str>) -> SessionId {
    let index = self.viewers.insert(ViewerData {
      session,
      name: name.map(|s| s.into()),
      watching: None,
    });
    SessionId(Role::Viewer, index)
  }

  /// Removes a session from the appropriate sparse set based on its role.
  pub fn remove_session(&self, session_id: SessionId) -> Option<Session> {
    match Role::from(session_id) {
      Role::Streamer => self.streamers.remove(*session_id).map(|data| data.session),
      Role::Controller => self
        .controllers
        .remove(*session_id)
        .map(|data| data.session),
      Role::Viewer => self.viewers.remove(*session_id).map(|data| data.session),
    }
  }

  /// Retrieves the session associated with a streamer by their index, returning `None` if the streamer does not exist.
  pub fn get_streamer_session(&self, streamer_index: usize) -> Option<StreamerSession> {
    self.streamers.view(streamer_index, |streamer| {
      StreamerSession::from(streamer.session.clone())
    })
  }

  /// Retrieves the session associated with a viewer by their index, returning `None` if the viewer does not exist.
  pub fn get_viewer_session(&self, viewer_index: usize) -> Option<ViewerSession> {
    self.viewers.view(viewer_index, |viewer| {
      ViewerSession::from(viewer.session.clone())
    })
  }

  /// Retrieves the session associated with a controller by their index, returning `None` if the controller does not exist.
  pub fn get_controller_session(&self, controller_index: usize) -> Option<ControllerSession> {
    self.controllers.view(controller_index, |controller| {
      ControllerSession::from(controller.session.clone())
    })
  }

  /// Retrieves all controller sessions.
  pub fn get_all_controller_sessions(&self) -> Vec<(usize, ControllerSession)> {
    self
      .controllers
      .map(|(i, v)| (i, ControllerSession::from(v.session.clone())))
  }

  /// Retrieves all viewer sessions that are currently watching a specific streamer, identified by their index.
  pub fn get_all_viewer_sessions_by_streamer(
    &self,
    streamer_index: Option<usize>,
  ) -> Vec<(usize, ViewerSession)> {
    self.viewers.filter_map(|(i, v)| {
      if v.watching == streamer_index {
        Some((i, ViewerSession::from(v.session.clone())))
      } else {
        None
      }
    })
  }

  /// Retrieves all streamer sessions in proto format for the controller.
  pub fn get_all_proto_streamer_sessions(&self) -> Vec<ProtoStreamerSession> {
    self
      .streamers
      .map(|(i, v)| ProtoStreamerSession::from((i, v)))
  }

  /// Retrieves the number of viewers currently connected to the server.
  pub fn len_viewers(&self) -> usize {
    self.viewers.len()
  }

  /// Retrieves all viewer sessions that are currently watching a specific streamer, identified by their index.
  pub fn get_proto_viewer_sessions_by_streamer(
    &self,
    streamer_index: Option<usize>,
  ) -> Vec<ProtoViewerSession> {
    self.viewers.filter_map(|(i, v)| {
      if v.watching == streamer_index {
        Some(ProtoViewerSession::from((i, v)))
      } else {
        None
      }
    })
  }

  /// Retrieves all viewer sessions in proto format for the controller.
  pub fn get_all_proto_viewer_sessions(&self) -> Vec<ProtoViewerSession> {
    self.viewers.map(|(i, v)| ProtoViewerSession::from((i, v)))
  }

  /// Updates the watching status of a viewer by their index, setting it to the specified streamer index or `None` if they are not watching any streamer.
  pub fn update_viewer_watching(&self, viewer_index: usize, streamer_index: Option<usize>) -> bool {
    self
      .viewers
      .update(viewer_index, |v| v.watching = streamer_index)
      .is_some()
  }

  /// Retrieves the streamer session in proto format for a specific streamer index, returning `None` if the streamer does not exist.
  pub fn get_proto_streamer_session(&self, streamer_index: usize) -> Option<ProtoStreamerSession> {
    self.streamers.view(streamer_index, |streamer| {
      ProtoStreamerSession::from((streamer_index, streamer))
    })
  }

  /// Retrieves the video transform for a specific streamer index, returning `None` if the streamer does not exist.
  pub fn get_streamer_video_transform(
    &self,
    streamer_index: usize,
  ) -> Option<Option<VideoTransform>> {
    self
      .streamers
      .view(streamer_index, |streamer| streamer.video_transform.clone())
  }

  /// Updates the video transform for a specific streamer index, returning `true` if the update was successful and `false` if the streamer does not exist.
  pub fn set_streamer_video_transform(
    &self,
    streamer_index: usize,
    video_transform: Option<VideoTransform>,
  ) -> bool {
    self
      .streamers
      .update(streamer_index, |s| s.video_transform = video_transform)
      .is_some()
  }

  /// Updates the battery level for a specific streamer index, returning `true` if the update was successful and `false` if the streamer does not exist.
  pub fn set_streamer_battery_level(
    &self,
    streamer_index: usize,
    battery_level: Option<u8>,
  ) -> bool {
    self
      .streamers
      .update(streamer_index, |s| s.battery_level = battery_level)
      .is_some()
  }

  /// Retrieves the streamer session in proto format for a specific streamer index, returning `None` if the streamer does not exist.
  pub fn get_proto_viewer_session(&self, viewer_index: usize) -> Option<ProtoViewerSession> {
    self.viewers.view(viewer_index, |viewer| {
      ProtoViewerSession::from((viewer_index, viewer))
    })
  }
}
