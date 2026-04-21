use std::env::current_exe;

use actix_files::NamedFile;
use actix_web::{
  HttpRequest, HttpResponse, Responder,
  body::{BoxBody, EitherBody},
  get, mime,
};

use crate::frontend;

#[get("/logo.png")]
#[tracing::instrument(skip_all, name = "logo.png")]
async fn logo_png(req: HttpRequest) -> HttpResponse<EitherBody<BoxBody>> {
  let file_path = current_exe()
    .ok()
    .and_then(|p| p.parent().map(|p| p.join("logo.png")));

  if let Some(path) = file_path
    && path.exists()
  {
    tracing::debug!("Serving logo from file system: {path:?}");
    NamedFile::open_async(path).await.respond_to(&req)
  } else {
    frontend::get("logo.png")
      .map(|content| {
        tracing::debug!("Serving logo from embedded resource");
        HttpResponse::Ok()
          .content_type(mime::IMAGE_PNG)
          .body(content)
          .map_into_left_body()
      })
      .unwrap_or_else(|| {
        tracing::debug!("File not found sending fallback");
        HttpResponse::Ok()
          .content_type(mime::IMAGE_PNG)
          .body(&include_bytes!("../static/logo.png")[..])
          .map_into_left_body()
      })
  }
}
