//! GIFTI XML reader.
//!
//! Parses a `.gii` file into a [`GiftiImage`] preserving all data arrays,
//! coordinate-system records, label tables, and metadata. The on-disk
//! payload encoding (ASCII / Base64 / GZipBase64) is decoded into a typed
//! [`ArrayData`] but the original `encoding` is retained on each
//! `DataArray` so the writer can round-trip it.

use std::fs;
use std::io::Read as _;
use std::path::Path;

use base64::Engine as _;
use roxmltree::{Document, Node};

use crate::error::{GiftiError, Result};
use crate::model::{
    ArrayData, ArrayIndexOrder, CoordSystem, DataArray, DataType, Encoding, Endian, GiftiImage,
    Label, LabelTable, Meta,
};

/// Parse a GIFTI file at `path`.
pub fn read(path: &Path) -> Result<GiftiImage> {
    let bytes = fs::read(path).map_err(|e| GiftiError::io(path, e))?;
    let xml = std::str::from_utf8(&bytes).map_err(|e| {
        GiftiError::fmt(format!("GIFTI file is not valid UTF-8: {e}"))
    })?;
    parse_str(xml)
}

/// Parse a GIFTI document from an in-memory XML string.
pub fn parse_str(xml: &str) -> Result<GiftiImage> {
    let doc = Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )?;

    let root = doc.root_element();
    if !root.has_tag_name("GIFTI") {
        return Err(GiftiError::fmt(format!(
            "expected root element <GIFTI>, got <{}>",
            root.tag_name().name()
        )));
    }

    let version = root.attribute("Version").unwrap_or("1.0").to_string();
    let num_data_arrays: i32 = root
        .attribute("NumberOfDataArrays")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1);

    let mut top_meta: Meta = Vec::new();
    let mut label_table: Option<LabelTable> = None;
    let mut data_arrays: Vec<DataArray> = Vec::new();

    for child in root.children().filter(Node::is_element) {
        match child.tag_name().name() {
            "MetaData" => top_meta = parse_meta(child),
            "LabelTable" => label_table = Some(parse_label_table(child)),
            "DataArray" => data_arrays.push(parse_data_array(child)?),
            _ => {}
        }
    }

    Ok(GiftiImage {
        version,
        num_data_arrays,
        meta: top_meta,
        label_table,
        data_arrays,
    })
}

fn parse_meta(node: Node<'_, '_>) -> Meta {
    let mut out = Vec::new();
    for md in node.children().filter(|n| n.has_tag_name("MD")) {
        let name = md
            .children()
            .find(|n| n.has_tag_name("Name"))
            .and_then(|n| n.text())
            .unwrap_or("")
            .to_string();
        let value = md
            .children()
            .find(|n| n.has_tag_name("Value"))
            .and_then(|n| n.text())
            .unwrap_or("")
            .to_string();
        out.push((name, value));
    }
    out
}

fn parse_label_table(node: Node<'_, '_>) -> LabelTable {
    let mut labels = Vec::new();
    for label in node.children().filter(|n| n.has_tag_name("Label")) {
        let key = label
            .attribute("Key")
            .or_else(|| label.attribute("Index"))
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(0);
        let red = label.attribute("Red").and_then(|s| s.parse().ok());
        let green = label.attribute("Green").and_then(|s| s.parse().ok());
        let blue = label.attribute("Blue").and_then(|s| s.parse().ok());
        let alpha = label.attribute("Alpha").and_then(|s| s.parse().ok());
        let text = label.text().unwrap_or("").to_string();
        labels.push(Label {
            key,
            red,
            green,
            blue,
            alpha,
            text,
        });
    }
    LabelTable { labels }
}

fn parse_data_array(da: Node<'_, '_>) -> Result<DataArray> {
    let intent = parse_intent(
        da.attribute("Intent")
            .ok_or_else(|| GiftiError::fmt("DataArray missing Intent attribute"))?,
    )?;
    let datatype_code = parse_datatype_code(
        da.attribute("DataType")
            .ok_or_else(|| GiftiError::fmt("DataArray missing DataType attribute"))?,
    )?;
    let dtype = DataType::from_code(datatype_code).ok_or_else(|| {
        GiftiError::fmt(format!("unsupported NIFTI DataType code: {datatype_code}"))
    })?;
    let encoding = parse_encoding(
        da.attribute("Encoding")
            .ok_or_else(|| GiftiError::fmt("DataArray missing Encoding attribute"))?,
    )?;
    let endian = parse_endian(da.attribute("Endian").unwrap_or("LittleEndian"))?;
    let array_index_order = parse_index_order(
        da.attribute("ArrayIndexingOrder")
            .unwrap_or("RowMajorOrder"),
    )?;
    let dims = parse_dims(da)?;

    let mut coordsys = Vec::new();
    let mut meta: Meta = Vec::new();
    let mut data_text = String::new();

    for child in da.children().filter(Node::is_element) {
        match child.tag_name().name() {
            "MetaData" => meta = parse_meta(child),
            "CoordinateSystemTransformMatrix" => coordsys.push(parse_coord_system(child)),
            "Data" => data_text = child.text().unwrap_or("").to_string(),
            _ => {}
        }
    }

    let ext_filename = da
        .attribute("ExternalFileName")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let ext_offset = da
        .attribute("ExternalFileOffset")
        .and_then(|s| s.parse().ok());

    let total: usize = dims.iter().copied().product();
    let data = match encoding {
        Encoding::ExternalFileBinary => {
            return Err(GiftiError::fmt(
                "ExternalFileBinary GIFTI encoding is not supported",
            ));
        }
        Encoding::Ascii => decode_ascii(&data_text, dtype, total)?,
        Encoding::Base64Binary | Encoding::GZipBase64Binary => {
            let bytes = decode_b64_payload(&data_text, encoding)?;
            decode_binary(&bytes, dtype, endian, total)?
        }
    };

    Ok(DataArray {
        intent,
        datatype: datatype_code,
        array_index_order,
        dims,
        encoding,
        endian,
        ext_filename,
        ext_offset,
        coordsys,
        meta,
        data,
    })
}

fn parse_coord_system(node: Node<'_, '_>) -> CoordSystem {
    let data_space = node
        .children()
        .find(|n| n.has_tag_name("DataSpace"))
        .and_then(|n| n.text())
        .unwrap_or("NIFTI_XFORM_UNKNOWN")
        .to_string();
    let transformed_space = node
        .children()
        .find(|n| n.has_tag_name("TransformedSpace"))
        .and_then(|n| n.text())
        .unwrap_or("NIFTI_XFORM_UNKNOWN")
        .to_string();
    let mut xform = CoordSystem::identity();
    if let Some(matrix_node) = node.children().find(|n| n.has_tag_name("MatrixData")) {
        let text = matrix_node.text().unwrap_or("");
        let nums: Vec<f64> = text
            .split_whitespace()
            .filter_map(|t| t.parse::<f64>().ok())
            .collect();
        if nums.len() == 16 {
            for i in 0..4 {
                for j in 0..4 {
                    xform[i][j] = nums[i * 4 + j];
                }
            }
        }
    }
    CoordSystem {
        data_space,
        transformed_space,
        xform,
    }
}

fn parse_intent(intent: &str) -> Result<i32> {
    if let Ok(v) = intent.parse::<i32>() {
        return Ok(v);
    }
    if let Some(code) = crate::intent::code_for_name(intent) {
        return Ok(code);
    }
    Err(GiftiError::fmt(format!(
        "unrecognized DataArray Intent: {intent}"
    )))
}

fn parse_datatype_code(s: &str) -> Result<i32> {
    if let Ok(v) = s.parse::<i32>() {
        return Ok(v);
    }
    DataType::from_name(s)
        .map(|d| d as i32)
        .ok_or_else(|| GiftiError::fmt(format!("unrecognized DataType: {s}")))
}

fn parse_encoding(s: &str) -> Result<Encoding> {
    Ok(match s {
        "ASCII" => Encoding::Ascii,
        "Base64Binary" => Encoding::Base64Binary,
        "GZipBase64Binary" => Encoding::GZipBase64Binary,
        "ExternalFileBinary" => Encoding::ExternalFileBinary,
        other => return Err(GiftiError::fmt(format!("unsupported Encoding: {other}"))),
    })
}

fn parse_endian(s: &str) -> Result<Endian> {
    Ok(match s {
        "LittleEndian" => Endian::Little,
        "BigEndian" => Endian::Big,
        // Some pipeline outputs use "Undefined"; treat as little-endian.
        "Undefined" => Endian::Little,
        other => return Err(GiftiError::fmt(format!("unsupported Endian: {other}"))),
    })
}

fn parse_index_order(s: &str) -> Result<ArrayIndexOrder> {
    Ok(match s {
        "RowMajorOrder" => ArrayIndexOrder::RowMajor,
        "ColumnMajorOrder" => ArrayIndexOrder::ColumnMajor,
        "Undefined" => ArrayIndexOrder::RowMajor,
        other => {
            return Err(GiftiError::fmt(format!(
                "unsupported ArrayIndexingOrder: {other}"
            )))
        }
    })
}

fn parse_dims(da: Node<'_, '_>) -> Result<Vec<usize>> {
    let ndim: usize = da
        .attribute("Dimensionality")
        .ok_or_else(|| GiftiError::fmt("DataArray missing Dimensionality"))?
        .parse()
        .map_err(|e| GiftiError::fmt(format!("invalid Dimensionality: {e}")))?;
    let mut dims = Vec::with_capacity(ndim);
    for i in 0..ndim {
        let key = format!("Dim{i}");
        let d: usize = da
            .attribute(key.as_str())
            .ok_or_else(|| GiftiError::fmt(format!("DataArray missing {key}")))?
            .parse()
            .map_err(|e| GiftiError::fmt(format!("invalid {key}: {e}")))?;
        dims.push(d);
    }
    Ok(dims)
}

fn decode_b64_payload(text: &str, encoding: Encoding) -> Result<Vec<u8>> {
    let compact: String = text
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    let raw = base64::engine::general_purpose::STANDARD.decode(compact.as_bytes())?;
    if !matches!(encoding, Encoding::GZipBase64Binary) {
        return Ok(raw);
    }
    // Some files labelled GZipBase64Binary contain raw zlib or deflate
    // streams instead of a true gzip wrapper. Try gzip first, fall back
    // to zlib, then raw deflate, mirroring the existing TRXViz logic.
    let mut out = Vec::new();
    {
        let mut dec = flate2::read::GzDecoder::new(raw.as_slice());
        if dec.read_to_end(&mut out).is_ok() {
            return Ok(out);
        }
    }
    out.clear();
    {
        let mut dec = flate2::read::ZlibDecoder::new(raw.as_slice());
        if dec.read_to_end(&mut out).is_ok() {
            return Ok(out);
        }
    }
    out.clear();
    let mut dec = flate2::read::DeflateDecoder::new(raw.as_slice());
    dec.read_to_end(&mut out).map_err(GiftiError::Decompress)?;
    Ok(out)
}

fn decode_ascii(text: &str, dtype: DataType, expected: usize) -> Result<ArrayData> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    if expected != 0 && tokens.len() != expected {
        return Err(GiftiError::fmt(format!(
            "ASCII DataArray has {} tokens, expected {expected}",
            tokens.len()
        )));
    }
    macro_rules! parse_into {
        ($vec:ident, $ty:ty) => {{
            let mut $vec = Vec::with_capacity(tokens.len());
            for tok in &tokens {
                let v: $ty = tok.parse().map_err(|e| {
                    GiftiError::fmt(format!("invalid ASCII numeric token '{tok}': {e}"))
                })?;
                $vec.push(v);
            }
            $vec
        }};
    }
    Ok(match dtype {
        DataType::UInt8 => ArrayData::UInt8(parse_into!(v, u8)),
        DataType::Int8 => ArrayData::Int8(parse_into!(v, i8)),
        DataType::UInt16 => ArrayData::UInt16(parse_into!(v, u16)),
        DataType::Int16 => ArrayData::Int16(parse_into!(v, i16)),
        DataType::UInt32 => ArrayData::UInt32(parse_into!(v, u32)),
        DataType::Int32 => ArrayData::Int32(parse_into!(v, i32)),
        DataType::UInt64 => ArrayData::UInt64(parse_into!(v, u64)),
        DataType::Int64 => ArrayData::Int64(parse_into!(v, i64)),
        DataType::Float32 => ArrayData::Float32(parse_into!(v, f32)),
        DataType::Float64 => ArrayData::Float64(parse_into!(v, f64)),
    })
}

fn decode_binary(
    bytes: &[u8],
    dtype: DataType,
    endian: Endian,
    expected: usize,
) -> Result<ArrayData> {
    let elem = dtype.elem_size();
    if bytes.len() % elem != 0 {
        return Err(GiftiError::fmt(format!(
            "binary DataArray has {} bytes, not a multiple of {elem}",
            bytes.len()
        )));
    }
    let count = bytes.len() / elem;
    if expected != 0 && count != expected {
        return Err(GiftiError::fmt(format!(
            "binary DataArray has {count} elements, expected {expected}"
        )));
    }

    macro_rules! decode {
        ($ty:ty, $size:expr, $from_le:expr, $from_be:expr) => {{
            let mut out: Vec<$ty> = Vec::with_capacity(count);
            for chunk in bytes.chunks_exact($size) {
                let arr: [u8; $size] = chunk.try_into().unwrap();
                let v = match endian {
                    Endian::Little => $from_le(arr),
                    Endian::Big => $from_be(arr),
                };
                out.push(v);
            }
            out
        }};
    }

    Ok(match dtype {
        DataType::UInt8 => ArrayData::UInt8(bytes.to_vec()),
        DataType::Int8 => ArrayData::Int8(bytes.iter().map(|&b| b as i8).collect()),
        DataType::UInt16 => ArrayData::UInt16(decode!(u16, 2, u16::from_le_bytes, u16::from_be_bytes)),
        DataType::Int16 => ArrayData::Int16(decode!(i16, 2, i16::from_le_bytes, i16::from_be_bytes)),
        DataType::UInt32 => ArrayData::UInt32(decode!(u32, 4, u32::from_le_bytes, u32::from_be_bytes)),
        DataType::Int32 => ArrayData::Int32(decode!(i32, 4, i32::from_le_bytes, i32::from_be_bytes)),
        DataType::UInt64 => ArrayData::UInt64(decode!(u64, 8, u64::from_le_bytes, u64::from_be_bytes)),
        DataType::Int64 => ArrayData::Int64(decode!(i64, 8, i64::from_le_bytes, i64::from_be_bytes)),
        DataType::Float32 => ArrayData::Float32(decode!(f32, 4, f32::from_le_bytes, f32::from_be_bytes)),
        DataType::Float64 => ArrayData::Float64(decode!(f64, 8, f64::from_le_bytes, f64::from_be_bytes)),
    })
}
