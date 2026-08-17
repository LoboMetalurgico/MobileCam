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
  routes::{
    color::color_txt, logo::logo_png, static_routes::index, version::version,
    websocket::incoming_socket,
  },
};

mod app_state;
mod proto;
mod routes;
mod server;
mod sparse_set;

mod frontend {
  include!(concat!(env!("OUT_DIR"), "/frontend.rs"));

  pub fn get(path: &str) -> Option<&'static [u8]> {
    FRONTEND.get(path).copied()
  }
}

const MSG_TIMEOUT: u64 = 5; // seconds
const CONN_TIMEOUT: u64 = 15; // seconds
const DEFAULT_PORT: u16 = 3000;

fn parse_port<I>(args: I) -> Result<u16, String>
where
  I: IntoIterator<Item = String>,
{
  let mut args = args.into_iter().peekable();
  let mut port = DEFAULT_PORT;

  while let Some(arg) = args.next() {
    match arg.as_str() {
      "--port" | "-p" => {
        let value = args.next().ok_or_else(|| {
          "Missing value for --port. Use --port <number>".to_string()
        })?;
        port = parse_single_port(&value)?;
      }
      value if value.starts_with("--port=") => {
        port = parse_single_port(value.strip_prefix("--port=").unwrap())?;
      }
      "--help" | "-h" => {
        return Ok(DEFAULT_PORT);
      }
      _ => {}
    }
  }

  Ok(port)
}

fn parse_single_port(raw: &str) -> Result<u16, String> {
  let raw = raw.trim();

  match raw.parse::<u16>() {
    Ok(value) if value != 0 => Ok(value),
    _ => Err(format!(
      "Invalid port: '{raw}'. Use a number between 1 and 65535."
    )),
  }
}

#[actix_web::main]
async fn main() {
  println!(include_str!("static/asciiart.txt"));
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

  let port = parse_port(std::env::args().skip(1).collect::<Vec<_>>())
    .unwrap_or_else(|err| {
      eprintln!("{err}");
      exit(1)
    });

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
          IpAddr::V4(ipv4) => {
            if !ipv4.is_unspecified() {
              tracing::info!("  https://{}:{port}", ipv4);
            }
          }
          IpAddr::V6(ipv6) => {
            if !ipv6.is_unspecified() && !ipv6.is_unicast_link_local() {
              tracing::info!("  https://[{}]:{port}", ipv6);
            }
          }
        };
      }
    }
    Err(e) => {
      tracing::error!("Failed to retrieve network interfaces: {e}");
      tracing::info!("Server is running on https://localhost:{port}");
    }
  }

  HttpServer::new(move || {
    App::new()
      .app_data(app_state.clone())
      .service(incoming_socket)
      .service(logo_png)
      .service(color_txt)
      .service(version)
      .service(index)
  })
  .bind_rustls_0_23(
    [
      SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, port, 0, 0)),
      SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port)),
    ]
    .as_slice(),
    tls_config,
  )
  .unwrap_or_else(|e| {
    tracing::error!("Failed to bind server to port {port}: {e}");
    exit(1);
  })
  .run()
  .await
  .unwrap_or_else(|e| {
    tracing::error!("Server failed to start: {e}");
    exit(1);
  });
}
