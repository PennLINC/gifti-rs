//! Structural model for a parsed GIFTI file.
//!
//! Mirrors the GIFTI 1.0 XML schema closely enough that read → write
//! round-trips preserve every field that downstream tools commonly read.
//! See [the GIFTI specification](https://www.nitrc.org/projects/gifti/)
//! for the authoritative definitions.

/// Ordered key/value metadata. GIFTI's `<MetaData>` blocks are ordered
/// and may contain duplicate keys (rare but legal), so we use a `Vec`
/// rather than a `HashMap`.
pub type Meta = Vec<(String, String)>;

/// Look up the first value for a metadata key, or `None`.
pub fn meta_get<'a>(meta: &'a Meta, key: &str) -> Option<&'a str> {
    meta.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// Insert or replace the value for a metadata key.
pub fn meta_set(meta: &mut Meta, key: &str, value: impl Into<String>) {
    let value = value.into();
    if let Some(pair) = meta.iter_mut().find(|(k, _)| k == key) {
        pair.1 = value;
    } else {
        meta.push((key.to_string(), value));
    }
}

/// `Endian` of a binary `DataArray` payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    Little,
    Big,
}

impl Endian {
    pub fn as_str(self) -> &'static str {
        match self {
            Endian::Little => "LittleEndian",
            Endian::Big => "BigEndian",
        }
    }

    pub(crate) fn native() -> Self {
        if cfg!(target_endian = "little") {
            Endian::Little
        } else {
            Endian::Big
        }
    }
}

/// `ArrayIndexingOrder` for an N-dimensional `DataArray`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayIndexOrder {
    RowMajor,
    ColumnMajor,
}

impl ArrayIndexOrder {
    pub fn as_str(self) -> &'static str {
        match self {
            ArrayIndexOrder::RowMajor => "RowMajorOrder",
            ArrayIndexOrder::ColumnMajor => "ColumnMajorOrder",
        }
    }
}

/// On-disk encoding of a `DataArray` payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Ascii,
    Base64Binary,
    GZipBase64Binary,
    /// Reserved — `gifti-rs` does not currently read or write this form
    /// (the payload would live in a separate file).
    ExternalFileBinary,
}

impl Encoding {
    pub fn as_str(self) -> &'static str {
        match self {
            Encoding::Ascii => "ASCII",
            Encoding::Base64Binary => "Base64Binary",
            Encoding::GZipBase64Binary => "GZipBase64Binary",
            Encoding::ExternalFileBinary => "ExternalFileBinary",
        }
    }
}

/// NIFTI datatype code attached to a `DataArray`. The integer form
/// matches `nifti1.h` and is preserved exactly in [`DataArray::datatype`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    UInt8 = 2,
    Int16 = 4,
    Int32 = 8,
    Float32 = 16,
    Float64 = 64,
    Int8 = 256,
    UInt16 = 512,
    UInt32 = 768,
    Int64 = 1024,
    UInt64 = 1280,
}

impl DataType {
    pub fn from_code(code: i32) -> Option<Self> {
        Some(match code {
            2 => DataType::UInt8,
            4 => DataType::Int16,
            8 => DataType::Int32,
            16 => DataType::Float32,
            64 => DataType::Float64,
            256 => DataType::Int8,
            512 => DataType::UInt16,
            768 => DataType::UInt32,
            1024 => DataType::Int64,
            1280 => DataType::UInt64,
            _ => return None,
        })
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "NIFTI_TYPE_UINT8" => DataType::UInt8,
            "NIFTI_TYPE_INT16" => DataType::Int16,
            "NIFTI_TYPE_INT32" => DataType::Int32,
            "NIFTI_TYPE_FLOAT32" => DataType::Float32,
            "NIFTI_TYPE_FLOAT64" => DataType::Float64,
            "NIFTI_TYPE_INT8" => DataType::Int8,
            "NIFTI_TYPE_UINT16" => DataType::UInt16,
            "NIFTI_TYPE_UINT32" => DataType::UInt32,
            "NIFTI_TYPE_INT64" => DataType::Int64,
            "NIFTI_TYPE_UINT64" => DataType::UInt64,
            _ => return None,
        })
    }

    pub fn as_name(self) -> &'static str {
        match self {
            DataType::UInt8 => "NIFTI_TYPE_UINT8",
            DataType::Int16 => "NIFTI_TYPE_INT16",
            DataType::Int32 => "NIFTI_TYPE_INT32",
            DataType::Float32 => "NIFTI_TYPE_FLOAT32",
            DataType::Float64 => "NIFTI_TYPE_FLOAT64",
            DataType::Int8 => "NIFTI_TYPE_INT8",
            DataType::UInt16 => "NIFTI_TYPE_UINT16",
            DataType::UInt32 => "NIFTI_TYPE_UINT32",
            DataType::Int64 => "NIFTI_TYPE_INT64",
            DataType::UInt64 => "NIFTI_TYPE_UINT64",
        }
    }

    pub fn elem_size(self) -> usize {
        match self {
            DataType::UInt8 | DataType::Int8 => 1,
            DataType::Int16 | DataType::UInt16 => 2,
            DataType::Int32 | DataType::UInt32 | DataType::Float32 => 4,
            DataType::Int64 | DataType::UInt64 | DataType::Float64 => 8,
        }
    }
}

/// Typed payload of a `DataArray`. The variant matches [`DataArray::dtype`].
#[derive(Debug, Clone)]
pub enum ArrayData {
    UInt8(Vec<u8>),
    Int8(Vec<i8>),
    UInt16(Vec<u16>),
    Int16(Vec<i16>),
    UInt32(Vec<u32>),
    Int32(Vec<i32>),
    UInt64(Vec<u64>),
    Int64(Vec<i64>),
    Float32(Vec<f32>),
    Float64(Vec<f64>),
}

impl ArrayData {
    pub fn dtype(&self) -> DataType {
        match self {
            ArrayData::UInt8(_) => DataType::UInt8,
            ArrayData::Int8(_) => DataType::Int8,
            ArrayData::UInt16(_) => DataType::UInt16,
            ArrayData::Int16(_) => DataType::Int16,
            ArrayData::UInt32(_) => DataType::UInt32,
            ArrayData::Int32(_) => DataType::Int32,
            ArrayData::UInt64(_) => DataType::UInt64,
            ArrayData::Int64(_) => DataType::Int64,
            ArrayData::Float32(_) => DataType::Float32,
            ArrayData::Float64(_) => DataType::Float64,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            ArrayData::UInt8(v) => v.len(),
            ArrayData::Int8(v) => v.len(),
            ArrayData::UInt16(v) => v.len(),
            ArrayData::Int16(v) => v.len(),
            ArrayData::UInt32(v) => v.len(),
            ArrayData::Int32(v) => v.len(),
            ArrayData::UInt64(v) => v.len(),
            ArrayData::Int64(v) => v.len(),
            ArrayData::Float32(v) => v.len(),
            ArrayData::Float64(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// One `<CoordinateSystemTransformMatrix>` record on a `DataArray`.
/// A `DataArray` can carry multiple coordsys records.
#[derive(Debug, Clone)]
pub struct CoordSystem {
    pub data_space: String,
    pub transformed_space: String,
    pub xform: [[f64; 4]; 4],
}

impl CoordSystem {
    pub fn identity() -> [[f64; 4]; 4] {
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }
}

/// One `<Label>` entry inside a top-level `<LabelTable>`.
#[derive(Debug, Clone)]
pub struct Label {
    pub key: i32,
    pub red: Option<f32>,
    pub green: Option<f32>,
    pub blue: Option<f32>,
    pub alpha: Option<f32>,
    pub text: String,
}

/// File-level `<LabelTable>` (used by `.label.gii`).
#[derive(Debug, Clone, Default)]
pub struct LabelTable {
    pub labels: Vec<Label>,
}

/// One `<DataArray>` record.
#[derive(Debug, Clone)]
pub struct DataArray {
    /// NIFTI intent code (e.g. 1008 for POINTSET). Not all codes have
    /// canonical names — see [`crate::intent`].
    pub intent: i32,
    /// NIFTI datatype code (e.g. 16 for FLOAT32). The actual storage
    /// type is also reflected in `data` (see [`ArrayData::dtype`]).
    pub datatype: i32,
    pub array_index_order: ArrayIndexOrder,
    pub dims: Vec<usize>,
    pub encoding: Encoding,
    pub endian: Endian,
    pub ext_filename: Option<String>,
    pub ext_offset: Option<i64>,
    pub coordsys: Vec<CoordSystem>,
    pub meta: Meta,
    pub data: ArrayData,
}

/// A complete in-memory GIFTI image.
#[derive(Debug, Clone)]
pub struct GiftiImage {
    pub version: String,
    pub num_data_arrays: i32,
    pub meta: Meta,
    pub label_table: Option<LabelTable>,
    pub data_arrays: Vec<DataArray>,
}

impl GiftiImage {
    /// Find the first `DataArray` with the given NIFTI intent code.
    pub fn find_array(&self, intent: i32) -> Option<&DataArray> {
        self.data_arrays.iter().find(|d| d.intent == intent)
    }

    /// Mutable counterpart of [`find_array`].
    pub fn find_array_mut(&mut self, intent: i32) -> Option<&mut DataArray> {
        self.data_arrays.iter_mut().find(|d| d.intent == intent)
    }
}
