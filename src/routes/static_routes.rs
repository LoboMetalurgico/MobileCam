use actix_web::{HttpResponse, Responder, get};

#[get("/yt/{path:.*}")]
async fn index() -> impl Responder {
  HttpResponse::Ok().body(include_str!("../static/index.html"))
}