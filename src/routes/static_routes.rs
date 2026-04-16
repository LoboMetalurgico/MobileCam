use actix_web::{HttpResponse, get, web};
use mime_guess::{MimeGuess, mime};

use crate::frontend;

#[get("/{path:.*}")]
#[tracing::instrument(skip_all, name = "static_file", fields(path = %path))]
async fn index(path: web::Path<String>) -> HttpResponse {
  frontend::get(&path).map(|c| (c, MimeGuess::from_path(&*path).first_or_octet_stream())).or_else(|| {
    let data = if path.ends_with("/") || path.is_empty() {
      tracing::trace!("Directory detected, attempting to serve index.html");
      frontend::get(&format!("{path}index.html"))
    } else {
      tracing::trace!("Non-directory detected, attempting to serve *.html or */index.html");
      frontend::get(&format!("{path}.html")).or_else(|| frontend::get(&format!("{path}/index.html")))
    };

    if data.is_none() {
      tracing::trace!("Static file not found");
    }

    data.map(|c| (c, mime::TEXT_HTML_UTF_8))
  }).map(|(content, content_type)| {
    tracing::info!("Serving static file with content type: {content_type}");
    HttpResponse::Ok()
      .content_type(content_type)
      .body(content)
  }).unwrap_or_else(|| {
    tracing::warn!("File not found");
    HttpResponse::NotFound().body("Not found")
  })
}