//! CamY helper for YoloLiv YoloCam S7 and S3.
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
// FFI, protobuf varints, and V4L2 lengths are truncated by construction.
// Product names in docs are not rustdoc items.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate
)]

pub mod client;
pub mod cmd;
pub mod discover;
pub mod error;
pub mod holders;
pub mod params;
pub mod pb;
pub mod preview;
pub mod recover;
pub mod usbwatch;
pub mod wire;

pub use error::{Error, Result};
