//! GIFTI XML writer.
//!
//! Serializes a [`GiftiImage`] back to a `.gii` file, preserving all
//! data arrays, metadata, label tables, and coordinate-system records.
//! The on-disk encoding for each `DataArray` is taken from
//! `DataArray::encoding`; ASCII arrays are written as ASCII, Base64 as
//! base64, and `GZipBase64Binary` as zlib-wrapped, base64-encoded binary
//! (the GIFTI ecosystem convention — nibabel, ANTs, and ConnectomeWB
//! all write zlib under that encoding name).
//!
//! The writer always serialises in native byte order (and updates the
//! `Endian` attribute accordingly) regardless of what was on the
//! `DataArray` when read — this matches what `gifticlib` does.

use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::Path;

use base64::Engine as _;
use flate2::{write::ZlibEncoder, Compression};

use crate::error::{GiftiError, Result};
use crate::model::{
    ArrayData, ArrayIndexOrder, CoordSystem, DataArray, Encoding, Endian, GiftiImage, LabelTable,
    Meta,
};

const GIFTI_DTD_NAME: &str = "GIFTI";
const GIFTI_DTD_SYSTEM: &str = "http://gifti.projects.nitrc.org/gifti.dtd";

/// Write a [`GiftiImage`] to `path`.
pub fn write(img: &GiftiImage, path: &Path) -> Result<()> {
    let xml = serialize(img)?;
    fs::write(path, xml.as_bytes()).map_err(|e| GiftiError::io(path, e))
}

/// Serialize a [`GiftiImage`] to an XML string.
pub fn serialize(img: &GiftiImage) -> Result<String> {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!(
        "<!DOCTYPE {GIFTI_DTD_NAME} SYSTEM \"{GIFTI_DTD_SYSTEM}\">\n"
    ));

    let n_arrays = if img.num_data_arrays >= 0 {
        img.num_data_arrays
    } else {
        img.data_arrays.len() as i32
    };

    writeln!(
        &mut out,
        "<GIFTI Version=\"{}\" NumberOfDataArrays=\"{}\">",
        xml_attr(&img.version),
        n_arrays
    )
    .ok();

    write_meta(&mut out, &img.meta, "   ");
    if let Some(table) = &img.label_table {
        write_label_table(&mut out, table, "   ");
    }
    for da in &img.data_arrays {
        write_data_array(&mut out, da, "   ")?;
    }

    out.push_str("</GIFTI>\n");
    Ok(out)
}

fn write_meta(out: &mut String, meta: &Meta, indent: &str) {
    if meta.is_empty() {
        writeln!(out, "{indent}<MetaData/>").ok();
        return;
    }
    writeln!(out, "{indent}<MetaData>").ok();
    for (name, value) in meta {
        writeln!(out, "{indent}   <MD>").ok();
        writeln!(
            out,
            "{indent}      <Name><![CDATA[{}]]></Name>",
            cdata_escape(name)
        )
        .ok();
        writeln!(
            out,
            "{indent}      <Value><![CDATA[{}]]></Value>",
            cdata_escape(value)
        )
        .ok();
        writeln!(out, "{indent}   </MD>").ok();
    }
    writeln!(out, "{indent}</MetaData>").ok();
}

fn write_label_table(out: &mut String, table: &LabelTable, indent: &str) {
    if table.labels.is_empty() {
        writeln!(out, "{indent}<LabelTable/>").ok();
        return;
    }
    writeln!(out, "{indent}<LabelTable>").ok();
    for label in &table.labels {
        let mut attrs = format!(" Key=\"{}\"", label.key);
        if let Some(r) = label.red {
            attrs.push_str(&format!(" Red=\"{r}\""));
        }
        if let Some(g) = label.green {
            attrs.push_str(&format!(" Green=\"{g}\""));
        }
        if let Some(b) = label.blue {
            attrs.push_str(&format!(" Blue=\"{b}\""));
        }
        if let Some(a) = label.alpha {
            attrs.push_str(&format!(" Alpha=\"{a}\""));
        }
        writeln!(
            out,
            "{indent}   <Label{attrs}><![CDATA[{}]]></Label>",
            cdata_escape(&label.text)
        )
        .ok();
    }
    writeln!(out, "{indent}</LabelTable>").ok();
}

fn write_data_array(out: &mut String, da: &DataArray, indent: &str) -> Result<()> {
    // Always serialise in native byte order — the writer transcodes
    // payloads to native endian below, then reflects that here.
    let endian = if matches!(da.encoding, Encoding::Ascii) {
        // ASCII has no endian; preserve whatever was there for round-trip.
        da.endian
    } else {
        Endian::native()
    };

    let intent_name = crate::intent::name_for_code(da.intent)
        .map(|s| s.to_string())
        .unwrap_or_else(|| da.intent.to_string());
    let dtype_name = crate::model::DataType::from_code(da.datatype)
        .map(|d| d.as_name().to_string())
        .unwrap_or_else(|| da.datatype.to_string());

    writeln!(out, "{indent}<DataArray").ok();
    writeln!(out, "{indent}   Intent=\"{}\"", xml_attr(&intent_name)).ok();
    writeln!(out, "{indent}   DataType=\"{}\"", xml_attr(&dtype_name)).ok();
    writeln!(
        out,
        "{indent}   ArrayIndexingOrder=\"{}\"",
        da.array_index_order.as_str()
    )
    .ok();
    writeln!(out, "{indent}   Dimensionality=\"{}\"", da.dims.len()).ok();
    for (i, d) in da.dims.iter().enumerate() {
        writeln!(out, "{indent}   Dim{i}=\"{d}\"").ok();
    }
    writeln!(out, "{indent}   Encoding=\"{}\"", da.encoding.as_str()).ok();
    writeln!(out, "{indent}   Endian=\"{}\"", endian.as_str()).ok();
    writeln!(
        out,
        "{indent}   ExternalFileName=\"{}\"",
        xml_attr(da.ext_filename.as_deref().unwrap_or(""))
    )
    .ok();
    writeln!(
        out,
        "{indent}   ExternalFileOffset=\"{}\">",
        da.ext_offset.unwrap_or(0)
    )
    .ok();

    write_meta(out, &da.meta, &format!("{indent}   "));

    for cs in &da.coordsys {
        write_coord_system(out, cs, &format!("{indent}   "));
    }

    let payload = encode_payload(
        &da.data,
        da.encoding,
        endian,
        &da.array_index_order,
        &da.dims,
    )?;
    writeln!(out, "{indent}   <Data>{payload}</Data>").ok();
    writeln!(out, "{indent}</DataArray>").ok();
    Ok(())
}

fn write_coord_system(out: &mut String, cs: &CoordSystem, indent: &str) {
    writeln!(out, "{indent}<CoordinateSystemTransformMatrix>").ok();
    writeln!(
        out,
        "{indent}   <DataSpace><![CDATA[{}]]></DataSpace>",
        cdata_escape(&cs.data_space)
    )
    .ok();
    writeln!(
        out,
        "{indent}   <TransformedSpace><![CDATA[{}]]></TransformedSpace>",
        cdata_escape(&cs.transformed_space)
    )
    .ok();
    writeln!(out, "{indent}   <MatrixData>").ok();
    for row in cs.xform.iter() {
        writeln!(
            out,
            "{indent}      {} {} {} {}",
            fmt_f64(row[0]),
            fmt_f64(row[1]),
            fmt_f64(row[2]),
            fmt_f64(row[3])
        )
        .ok();
    }
    writeln!(out, "{indent}   </MatrixData>").ok();
    writeln!(out, "{indent}</CoordinateSystemTransformMatrix>").ok();
}

fn encode_payload(
    data: &ArrayData,
    encoding: Encoding,
    endian: Endian,
    _order: &ArrayIndexOrder,
    _dims: &[usize],
) -> Result<String> {
    if matches!(encoding, Encoding::ExternalFileBinary) {
        return Err(GiftiError::fmt(
            "ExternalFileBinary GIFTI encoding is not supported by the writer",
        ));
    }

    if matches!(encoding, Encoding::Ascii) {
        return Ok(encode_ascii(data));
    }

    let bytes = encode_binary_bytes(data, endian);
    let raw = if matches!(encoding, Encoding::GZipBase64Binary) {
        // Despite the name, the GIFTI ecosystem (nibabel, ANTs, ConnectomeWB)
        // writes zlib-wrapped data under `GZipBase64Binary`, not gzip-wrapped.
        // Match that convention so files round-trip with the rest of the
        // ecosystem; the reader still accepts true gzip as a fallback.
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
        enc.write_all(&bytes).map_err(GiftiError::Decompress)?;
        enc.finish().map_err(GiftiError::Decompress)?
    } else {
        bytes
    };
    Ok(base64::engine::general_purpose::STANDARD.encode(&raw))
}

fn encode_ascii(data: &ArrayData) -> String {
    let mut s = String::new();
    macro_rules! join {
        ($v:expr) => {{
            for (i, x) in $v.iter().enumerate() {
                if i > 0 {
                    s.push(' ');
                }
                let _ = write!(&mut s, "{}", x);
            }
        }};
    }
    match data {
        ArrayData::UInt8(v) => join!(v),
        ArrayData::Int8(v) => join!(v),
        ArrayData::UInt16(v) => join!(v),
        ArrayData::Int16(v) => join!(v),
        ArrayData::UInt32(v) => join!(v),
        ArrayData::Int32(v) => join!(v),
        ArrayData::UInt64(v) => join!(v),
        ArrayData::Int64(v) => join!(v),
        ArrayData::Float32(v) => join!(v),
        ArrayData::Float64(v) => join!(v),
    }
    s
}

fn encode_binary_bytes(data: &ArrayData, endian: Endian) -> Vec<u8> {
    macro_rules! to_bytes {
        ($vec:expr, $size:expr, $to_le:expr, $to_be:expr) => {{
            let mut out = Vec::with_capacity($vec.len() * $size);
            for v in $vec {
                let bytes: [u8; $size] = match endian {
                    Endian::Little => $to_le(*v),
                    Endian::Big => $to_be(*v),
                };
                out.extend_from_slice(&bytes);
            }
            out
        }};
    }
    match data {
        ArrayData::UInt8(v) => v.clone(),
        ArrayData::Int8(v) => v.iter().map(|&b| b as u8).collect(),
        ArrayData::UInt16(v) => to_bytes!(v, 2, u16::to_le_bytes, u16::to_be_bytes),
        ArrayData::Int16(v) => to_bytes!(v, 2, i16::to_le_bytes, i16::to_be_bytes),
        ArrayData::UInt32(v) => to_bytes!(v, 4, u32::to_le_bytes, u32::to_be_bytes),
        ArrayData::Int32(v) => to_bytes!(v, 4, i32::to_le_bytes, i32::to_be_bytes),
        ArrayData::UInt64(v) => to_bytes!(v, 8, u64::to_le_bytes, u64::to_be_bytes),
        ArrayData::Int64(v) => to_bytes!(v, 8, i64::to_le_bytes, i64::to_be_bytes),
        ArrayData::Float32(v) => to_bytes!(v, 4, f32::to_le_bytes, f32::to_be_bytes),
        ArrayData::Float64(v) => to_bytes!(v, 8, f64::to_le_bytes, f64::to_be_bytes),
    }
}

fn xml_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn cdata_escape(s: &str) -> String {
    // CDATA cannot contain "]]>". Split such occurrences across two
    // CDATA sections. Rare in practice for GIFTI metadata.
    s.replace("]]>", "]]]]><![CDATA[>")
}

fn fmt_f64(v: f64) -> String {
    // Match `printf("%f")` — six fractional digits — for parity with
    // the C++ ANTs writer.
    format!("{v:.6}")
}
