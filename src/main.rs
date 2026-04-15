use actix_web::{App, HttpServer, web::Data};
use rcgen::generate_simple_self_signed;
use rustls::ServerConfig;
use std::{env, process::exit};

use crate::{
  app_state::AppState,
  routes::{static_routes::index, websocket::incoming_socket},
};

mod app_state;
mod routes;
mod server;
mod commands;

mod frontend {
  include!(concat!(env!("OUT_DIR"), "/frontend.rs"));

  pub fn get(path: &str) -> Option<&'static [u8]> {
    FRONTEND.get(path).copied()
  }
}

const MSG_TIMEOUT: u64 = 5; // seconds
const CONN_TIMEOUT: u64 = 15; // seconds

fn log(text: &str, spacers: Option<(char, char)>) {
  let (start_border, end_border) = spacers.unwrap_or(('┃', '┃'));
  let inner_padding = if spacers.is_none() { " " } else { "" };
  let line = format!(
    "{}{}{:<87}{}{}",
    start_border, inner_padding, text, inner_padding, end_border
  );

  let term = env::var("TERM").unwrap_or_default();
  let is_windows = cfg!(windows);
  let supports_color = !is_windows || (!term.is_empty() && term != "dumb");
  if !supports_color {
    println!("{}", line);
    return;
  }

  let points = [
    (255.0, 0.0, 0.0),     // Red
    (255.0, 165.0, 0.0),   // Orange
    (255.0, 255.0, 0.0),   // Yellow
    (0.0, 255.0, 0.0),     // Green
    (0.0, 0.0, 255.0),     // Blue
    (75.0, 0.0, 130.0),    // Indigo
    (238.0, 130.0, 238.0), // Violet
  ];

  let len = line.chars().count();
  for (i, c) in line.chars().enumerate() {
    // Calculate where we are(from 0.0 to 1.0)
    let t = i as f32 / (len - 1) as f32;

    // Find which two colors we are between
    let scaled_t = t * (points.len() - 1) as f32;
    let idx = scaled_t.floor() as usize;
    let next_idx = (idx + 1).min(points.len() - 1);
    let lerp_t = scaled_t - idx as f32;

    // Blend
    let r = points[idx].0 + (points[next_idx].0 - points[idx].0) * lerp_t;
    let g = points[idx].1 + (points[next_idx].1 - points[idx].1) * lerp_t;
    let b = points[idx].2 + (points[next_idx].2 - points[idx].2) * lerp_t;

    print!("\x1b[38;2;{};{};{}m{}\x1b[0m", r as u8, g as u8, b as u8, c);
  }
  println!();
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  log(&format!("{:━<89}", ""), Some(('┏', '┓')));
  include_str!("static/asciiart.txt")
    .split('\n')
    .for_each(|line| {
      log(&format!(" {line}"), None);
    });
  log("", None);
  log(&format!("{:━<89}", ""), Some(('┣', '┫')));
  log("", None);
  log("MobileCam master server at: https://localhost:3000", None);
  log("", None);
  log(
    &format!("━[Application Logs]{:━<70}", ""),
    Some(('┣', '┫')),
  );

  let (cert, key) = generate_simple_self_signed(&[]).map_or_else(|e| {
    log(&format!("Failed to generate TLS certificate: {e}"), None);
    exit(1) 
  }, |c| (c.cert.into(), c.signing_key.into()));

  let tls_config = ServerConfig::builder()
    .with_no_client_auth()
    .with_single_cert(vec![cert], key)
    .unwrap_or_else(|e| {
      log(&format!("Failed to create TLS config: {e}"), None);
      exit(1)
    });

  HttpServer::new(|| {
    App::new()
      .app_data(Data::new(AppState::new()))
      .service(incoming_socket)
      .service(index)
  })
  .bind_rustls_0_23("0.0.0.0:3000", tls_config)?
  .run()
  .await
}
