use std::path::Path;

use anyhow::{Context, Result};

use gifti_rs::cras::{has_cras, read_cras};
use gifti_rs::intent;
use gifti_rs::{ArrayData, DataArray, DataType, GiftiImage};

pub fn run(path: &Path) -> Result<()> {
    let img = gifti_rs::read(path)
        .with_context(|| format!("failed to read GIFTI file: {}", path.display()))?;
    print_summary(&img, path);
    Ok(())
}

fn print_summary(img: &GiftiImage, path: &Path) {
    println!("File: {}", path.display());
    println!("GIFTI Version: {}", img.version);
    println!("Number of DataArrays: {}", img.data_arrays.len());

    if !img.meta.is_empty() {
        println!();
        println!("File metadata:");
        for (k, v) in &img.meta {
            println!("  {k} = {}", truncate(v, 120));
        }
    }

    if let Some(table) = &img.label_table {
        println!();
        println!("LabelTable: {} labels", table.labels.len());
    }

    for (i, da) in img.data_arrays.iter().enumerate() {
        println!();
        print_data_array(i, da);
    }
}

fn print_data_array(idx: usize, da: &DataArray) {
    let intent_name = intent::name_for_code(da.intent)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("(unnamed code {})", da.intent));
    let dtype_name = DataType::from_code(da.datatype)
        .map(|d| d.as_name().to_string())
        .unwrap_or_else(|| format!("code {}", da.datatype));
    let dims_str = da
        .dims
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(" x ");

    println!(
        "DataArray {idx}: intent={} ({}) datatype={} dims=[{dims_str}] encoding={} endian={} elements={}",
        intent_name,
        da.intent,
        dtype_name,
        da.encoding.as_str(),
        da.endian.as_str(),
        da.data.len(),
    );
    if !da.coordsys.is_empty() {
        println!("  CoordSystems: {}", da.coordsys.len());
        for (j, cs) in da.coordsys.iter().enumerate() {
            println!("    [{j}] {} -> {}", cs.data_space, cs.transformed_space);
            for row in cs.xform.iter() {
                println!(
                    "         {:>10.6} {:>10.6} {:>10.6} {:>10.6}",
                    row[0], row[1], row[2], row[3]
                );
            }
        }
    }
    if !da.meta.is_empty() {
        println!("  Metadata:");
        for (k, v) in &da.meta {
            println!("    {k} = {}", truncate(v, 120));
        }
    }

    if da.intent == intent::POINTSET {
        if let ArrayData::Float32(coords) = &da.data {
            print_pointset_extras(da, coords);
        }
    }
}

fn print_pointset_extras(da: &DataArray, coords: &[f32]) {
    let n_vertices = if da.dims.len() >= 2 && da.dims[1] == 3 {
        da.dims[0]
    } else {
        coords.len() / 3
    };
    println!("  POINTSET: {n_vertices} vertices");

    if !coords.is_empty() && coords.len() >= 3 {
        let mut bbox_min = [f32::INFINITY; 3];
        let mut bbox_max = [f32::NEG_INFINITY; 3];
        for chunk in coords.chunks_exact(3) {
            for j in 0..3 {
                if chunk[j] < bbox_min[j] {
                    bbox_min[j] = chunk[j];
                }
                if chunk[j] > bbox_max[j] {
                    bbox_max[j] = chunk[j];
                }
            }
        }
        println!(
            "    bbox min = ({:.4}, {:.4}, {:.4})",
            bbox_min[0], bbox_min[1], bbox_min[2]
        );
        println!(
            "    bbox max = ({:.4}, {:.4}, {:.4})",
            bbox_max[0], bbox_max[1], bbox_max[2]
        );
    }

    if has_cras(da) {
        let cras = read_cras(da);
        println!(
            "    C_RAS offset = ({:.6}, {:.6}, {:.6})  [VolGeomC_R/A/S]",
            cras[0], cras[1], cras[2]
        );
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max).collect();
        format!("{head}…")
    }
}
