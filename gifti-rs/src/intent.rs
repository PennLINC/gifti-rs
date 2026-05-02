//! NIFTI intent codes used in GIFTI `DataArray` headers.
//!
//! GIFTI files identify each `DataArray` by a NIFTI intent code (the same
//! codes used in the NIFTI-1 image format header). Only a small subset of
//! the codes are commonly seen in surface and label files; this module
//! enumerates the codes this crate recognizes by name and provides a
//! lossy `name → code` / `code → name` lookup.
//!
//! Codes that are not in this table are still parsed as integers — they
//! round-trip through reader/writer unchanged via the integer form on
//! `DataArray::intent`.

pub const POINTSET: i32 = 1008;
pub const TRIANGLE: i32 = 1009;
pub const TIME_SERIES: i32 = 2001;
pub const NODE_INDEX: i32 = 2002;
pub const RGB_VECTOR: i32 = 2003;
pub const RGBA_VECTOR: i32 = 2004;
pub const SHAPE: i32 = 2005;
pub const LABEL: i32 = 1002;
pub const NORMAL: i32 = 1007;
pub const VECTOR: i32 = 1006;
pub const NONE: i32 = 0;

/// Returns the canonical `NIFTI_INTENT_*` name for a code, or `None` if
/// the code is not in the table this crate knows about.
pub fn name_for_code(code: i32) -> Option<&'static str> {
    Some(match code {
        POINTSET => "NIFTI_INTENT_POINTSET",
        TRIANGLE => "NIFTI_INTENT_TRIANGLE",
        TIME_SERIES => "NIFTI_INTENT_TIME_SERIES",
        NODE_INDEX => "NIFTI_INTENT_NODE_INDEX",
        RGB_VECTOR => "NIFTI_INTENT_RGB_VECTOR",
        RGBA_VECTOR => "NIFTI_INTENT_RGBA_VECTOR",
        SHAPE => "NIFTI_INTENT_SHAPE",
        LABEL => "NIFTI_INTENT_LABEL",
        NORMAL => "NIFTI_INTENT_NORMAL",
        VECTOR => "NIFTI_INTENT_VECTOR",
        NONE => "NIFTI_INTENT_NONE",
        _ => return None,
    })
}

/// Inverse of [`name_for_code`]: returns the integer code for a
/// `NIFTI_INTENT_*` name string, or `None` if the name is not recognized.
pub fn code_for_name(name: &str) -> Option<i32> {
    Some(match name {
        "NIFTI_INTENT_POINTSET" => POINTSET,
        "NIFTI_INTENT_TRIANGLE" => TRIANGLE,
        "NIFTI_INTENT_TIME_SERIES" => TIME_SERIES,
        "NIFTI_INTENT_NODE_INDEX" => NODE_INDEX,
        "NIFTI_INTENT_RGB_VECTOR" => RGB_VECTOR,
        "NIFTI_INTENT_RGBA_VECTOR" => RGBA_VECTOR,
        "NIFTI_INTENT_SHAPE" => SHAPE,
        "NIFTI_INTENT_LABEL" => LABEL,
        "NIFTI_INTENT_NORMAL" => NORMAL,
        "NIFTI_INTENT_VECTOR" => VECTOR,
        "NIFTI_INTENT_NONE" => NONE,
        _ => return None,
    })
}
