pub mod common {
  include!(concat!(env!("OUT_DIR"), "/common.rs"));
}
pub mod controller {
  include!(concat!(env!("OUT_DIR"), "/controller.rs"));
}
pub mod streamer {
  include!(concat!(env!("OUT_DIR"), "/streamer.rs"));
}
pub mod viewer {
  include!(concat!(env!("OUT_DIR"), "/viewer.rs"));
}
