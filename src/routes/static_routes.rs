use actix_web::{HttpResponse, get, web};
use mime_guess::{MimeGuess};

use crate::frontend;

#[get("/{path:.*}")]
async fn index(path: web::Path<String>) -> HttpResponse {
  frontend::get(&path).map(|c| (c, MimeGuess::from_path(&*path).first_or_octet_stream())).or_else(|| {
    let data = if path.ends_with("/") || path.is_empty() {
     frontend::get(&format!("{path}index.html"))
    } else {
      frontend::get(&format!("{path}.html")).or_else(|| frontend::get(&format!("{path}/index.html")))
    };

    data.map(|c| (c, mime::TEXT_HTML_UTF_8))
  }).map(|(content, content_type)| {
    HttpResponse::Ok()
      .content_type(content_type)
      .body(content)
  }).unwrap_or_else(|| HttpResponse::NotFound().body("Not found"))
}