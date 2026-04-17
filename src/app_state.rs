use std::{
  hash::BuildHasherDefault,
  sync::atomic::{AtomicU8, Ordering},
};

use actix_ws::Session;
use dashmap::DashMap;
use fxhash::FxHasher32;
use parking_lot::Mutex;

use crate::server::{Roles, UserData};

pub struct AppState {
  trash_bin: Mutex<Vec<u8>>,
  connections: DashMap<u8, UserData, BuildHasherDefault<FxHasher32>>,
  last_session: AtomicU8,
}

impl AppState {
  pub fn new() -> Self {
    Self {
      last_session: AtomicU8::new(0),
      trash_bin: Mutex::new(Vec::with_capacity(1)),
      connections: DashMap::with_capacity_and_hasher(1, BuildHasherDefault::new()),
    }
  }

  pub fn discard(&self, id: u8) {
    tracing::debug!("Discarding session with ID {id}");
    self.trash_bin.lock().push(id);
    self.connections.remove(&id);
  }

  pub fn recycle(&self) -> Option<u8> {
    tracing::debug!("Attempting to get a recycled session ID");
    self.trash_bin.lock().pop().or_else(|| {
      self
        .last_session
        .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |old_value| {
          old_value.checked_add(1)
        })
        .ok()
    })
  }

  pub fn register(&self, id: u8, session: Session, role: Roles) {
    tracing::debug!("Registering new session with ID {id} and role {role:?}");
    self.connections.insert(id, UserData { session, role });
  }

  pub fn get_connection(&self, id: u8) -> Option<Session> {
    self.connections.get(&id).map(|item| item.session.clone())
  }

  pub fn get_conns<F: Fn(&UserData) -> bool>(&self, filter: F) -> Vec<Session> {
    let mut conns = Vec::new();
    for conn in self.connections.iter() {
      if filter(conn.value()) {
        conns.push(conn.value().session.clone());
      }
    }
    conns
  }

  pub fn get_ids<F: Fn(&UserData) -> bool>(&self, filter: F) -> Vec<u8> {
    let mut parts = Vec::new();
    for u_data in self.connections.iter() {
      if filter(u_data.value()) {
        parts.push(*u_data.key());
      }
    }

    parts
  }
}
