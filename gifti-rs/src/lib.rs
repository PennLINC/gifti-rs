//! Pure-Rust reader and writer for GIFTI surface and shape files.
//!
//! See the crate `README.md` for a high-level overview. The two
//! top-level entry points are:
//!
//! ```no_run
//! use gifti_rs::{read, write};
//! # fn ex() -> gifti_rs::Result<()> {
//! let img = read("lh.pial.surf.gii".as_ref())?;
//! write(&img, "out.surf.gii".as_ref())?;
//! # Ok(()) }
//! ```
//!
//! The on-disk model is exposed via [`GiftiImage`] and [`DataArray`];
//! see [`crate::cras`] for FreeSurfer C_RAS offset helpers used by the
//! `giftirs transform` CLI.

#![warn(rust_2018_idioms)]

pub mod cras;
pub mod error;
pub mod intent;
pub mod model;
pub mod reader;
pub mod writer;

pub use crate::error::{GiftiError, Result};
pub use crate::model::{
    meta_get, meta_set, ArrayData, ArrayIndexOrder, CoordSystem, DataArray, DataType, Encoding,
    Endian, GiftiImage, Label, LabelTable, Meta,
};
pub use crate::reader::{parse_str, read};
pub use crate::writer::{serialize, write};
