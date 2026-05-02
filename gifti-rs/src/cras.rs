//! FreeSurfer C_RAS offset helpers.
//!
//! When `mris_convert` writes a FreeSurfer surface as a GIFTI file, the
//! vertex coordinates are stored in FreeSurfer's "native surface RAS"
//! frame, which is offset from world RAS by a translation called C_RAS.
//! The offset is stored as three metadata key/value pairs on the
//! POINTSET data array:
//!
//! ```text
//! VolGeomC_R = <r-component>
//! VolGeomC_A = <a-component>
//! VolGeomC_S = <s-component>
//! ```
//!
//! World RAS = native surface RAS + C_RAS. To prepare vertices for any
//! transform that operates in world space, add the offset to each
//! coordinate. Once baked in, zero the metadata so downstream tools do
//! not double-apply it. This matches the behaviour of niworkflows
//! `normalize_surfs` and the C++ `antsApplyTransformsToGifti`.

use crate::intent;
use crate::model::{meta_get, meta_set, DataArray, GiftiImage};

const C_RAS_KEYS: [&str; 3] = ["VolGeomC_R", "VolGeomC_A", "VolGeomC_S"];

/// Read the C_RAS offset from a `DataArray`'s metadata. Missing or
/// unparseable components are returned as zero, matching the C++ tool.
pub fn read_cras(da: &DataArray) -> [f64; 3] {
    let mut out = [0.0; 3];
    for (i, key) in C_RAS_KEYS.iter().enumerate() {
        if let Some(v) = meta_get(&da.meta, key) {
            if let Ok(parsed) = v.parse::<f64>() {
                out[i] = parsed;
            }
        }
    }
    out
}

/// Returns `true` if any of the three `VolGeomC_R/A/S` keys is present
/// **and** non-zero.
pub fn has_cras(da: &DataArray) -> bool {
    let v = read_cras(da);
    v[0] != 0.0 || v[1] != 0.0 || v[2] != 0.0
}

/// Set `VolGeomC_R/A/S` to `"0.000000"` on a `DataArray`. Does not add
/// the keys if they were absent — matches the C++ tool, which only
/// rewrites keys that already exist.
pub fn zero_cras_meta(da: &mut DataArray) {
    for key in C_RAS_KEYS {
        if meta_get(&da.meta, key).is_some() {
            meta_set(&mut da.meta, key, "0.000000");
        }
    }
}

/// Find the POINTSET `DataArray` in an image, by intent code.
pub fn pointset_array(img: &GiftiImage) -> Option<&DataArray> {
    img.find_array(intent::POINTSET)
}

/// Mutable counterpart of [`pointset_array`].
pub fn pointset_array_mut(img: &mut GiftiImage) -> Option<&mut DataArray> {
    img.find_array_mut(intent::POINTSET)
}
