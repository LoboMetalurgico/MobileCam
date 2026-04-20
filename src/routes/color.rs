use std::env::current_exe;

use actix_files::NamedFile;
use actix_web::{
  HttpRequest, HttpResponse, Responder,
  body::{BoxBody, EitherBody},
  get, mime,
};

use crate::frontend;

#[get("/color.txt")]
#[tracing::instrument(skip_all, name = "color.txt")]
async fn color_txt(req: HttpRequest) -> HttpResponse<EitherBody<BoxBody>> {
  let file_path = current_exe()
    .ok()
    .and_then(|p| p.parent().map(|p| p.join("color.txt")));

  if let Some(path) = file_path
    && path.exists()
  {
    tracing::debug!("Serving color.txt from file system: {path:?}");
    NamedFile::open_async(path).await.respond_to(&req)
  } else {
    frontend::get("color.txt")
      .map(|content| {
        tracing::debug!("Serving color.txt from embedded resource");
        HttpResponse::Ok()
          .content_type(mime::TEXT_PLAIN)
          .body(content)
          .map_into_left_body()
      })
      .unwrap_or_else(|| {
        tracing::debug!("File not found");
        HttpResponse::NotFound()
          .body("Not found")
          .map_into_right_body()
      })
  }
}
