use actix_web::{App, HttpServer, web::Data};
use if_addrs::get_if_addrs;
use rcgen::generate_simple_self_signed;
use rustls::ServerConfig;
use std::{
  net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
  process::exit,
};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
  app_state::AppState,
  routes::{static_routes::index, websocket::incoming_socket},
};

mod app_state;
mod commands;
mod routes;
mod server;

mod frontend {
  include!(concat!(env!("OUT_DIR"), "/frontend.rs"));

  pub fn get(path: &str) -> Option<&'static [u8]> {
    FRONTEND.get(path).copied()
  }
}

const MSG_TIMEOUT: u64 = 5; // seconds
const CONN_TIMEOUT: u64 = 15; // seconds

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  tracing_subscriber::registry()
    .with(fmt::layer())
    .with(
      EnvFilter::try_from_env("MOBILE_CAM_LOG").unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
          EnvFilter::new("mobile_cam=debug")
        } else {
          EnvFilter::new("mobile_cam=info")
        }
      }),
    )
    .init();

  let (cert, key) = generate_simple_self_signed(&[]).map_or_else(
    |e| {
      tracing::error!("Failed to generate TLS certificate: {e}");
      exit(1)
    },
    |c| (c.cert.into(), c.signing_key.into()),
  );

  let tls_config = ServerConfig::builder()
    .with_no_client_auth()
    .with_single_cert(vec![cert], key)
    .unwrap_or_else(|e| {
      tracing::error!("Failed to create TLS config: {e}");
      exit(1)
    });

  let app_state = Data::new(AppState::new());

  match get_if_addrs() {
    Ok(addrs) => {
      tracing::info!("Server is running on the following addresses:");
      for addr in addrs {
        match addr.ip() {
          IpAddr::V4(ipv4) => tracing::info!("  https://{}:3000", ipv4),
          IpAddr::V6(ipv6) => tracing::info!("  https://[{}]:3000", ipv6),
        }
      }
    }
    Err(e) => {
      tracing::error!("Failed to retrieve network interfaces: {e}");
      tracing::info!("Server is running on https://localhost:3000");
    }
  }

  HttpServer::new(move || {
    App::new()
      .app_data(app_state.clone())
      .service(incoming_socket)
      .service(index)
  })
  .bind_rustls_0_23(
    [
      SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 3000)),
      SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 3000, 0, 0)),
    ]
    .as_slice(),
    tls_config,
  )?
  .run()
  .await
}
