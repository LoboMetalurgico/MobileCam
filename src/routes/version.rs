use actix_web::{HttpResponse, get};

#[get("/version")]
async fn version() -> HttpResponse {
  tracing::debug!("Serving version");
  HttpResponse::Ok()
    .content_type(mime::TEXT_PLAIN)
    .body(env!("CARGO_PKG_VERSION"))
}
