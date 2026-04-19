use actix_web::web::Data;
use actix_ws::Session;

use crate::{app_state::AppState, server::Commands};

pub async fn b_command(app_state: &Data<AppState>, mut session: Session) {
  let ids = app_state.get_all_streamers_id();
  let _ = session.text(Commands::C { id: ids }.to_string()).await;
}
