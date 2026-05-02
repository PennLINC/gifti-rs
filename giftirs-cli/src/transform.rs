use std::path::Path;

use anyhow::{anyhow, Context, Result};

use gifti_rs::cras::{has_cras, pointset_array_mut, read_cras, zero_cras_meta};
use gifti_rs::model::CoordSystem;
use gifti_rs::{ArrayData, GiftiImage};
use itk_transforms_rs::TransformChain;

pub fn run(input: &Path, output: &Path, transform: &Path, overwrite: bool) -> Result<()> {
    if output.exists() && !overwrite {
        return Err(anyhow!(
            "output file {} already exists (pass --overwrite to replace)",
            output.display()
        ));
    }

    let chain = itk_transforms_rs::read_itk(transform).map_err(|e| {
        anyhow!(
            "failed to read transform {}: {e}",
            transform.display()
        )
    })?;

    let mut img = gifti_rs::read(input)
        .with_context(|| format!("failed to read GIFTI file {}", input.display()))?;

    apply_transform_in_place(&mut img, &chain)?;

    gifti_rs::write(&img, output)
        .with_context(|| format!("failed to write {}", output.display()))?;

    Ok(())
}

fn apply_transform_in_place(img: &mut GiftiImage, chain: &TransformChain) -> Result<()> {
    let pointset = pointset_array_mut(img)
        .ok_or_else(|| anyhow!("input GIFTI has no NIFTI_INTENT_POINTSET DataArray"))?;

    if pointset.dims.len() < 2 || pointset.dims[1] != 3 {
        return Err(anyhow!(
            "POINTSET DataArray must have dimensions Nx3, got {:?}",
            pointset.dims
        ));
    }

    let cras = read_cras(pointset);
    let had_cras = has_cras(pointset);
    if had_cras {
        eprintln!(
            "Detected FreeSurfer C_RAS offset: R={:.6} A={:.6} S={:.6} (applying before transform).",
            cras[0], cras[1], cras[2]
        );
    }

    let coords = match &mut pointset.data {
        ArrayData::Float32(v) => v,
        other => {
            return Err(anyhow!(
                "POINTSET DataArray must be NIFTI_TYPE_FLOAT32 (got {:?})",
                other.dtype()
            ));
        }
    };

    if coords.len() % 3 != 0 {
        return Err(anyhow!(
            "POINTSET DataArray length {} is not a multiple of 3",
            coords.len()
        ));
    }

    let n_vertices = coords.len() / 3;
    for i in 0..n_vertices {
        let base = i * 3;
        let x = coords[base] as f64 + cras[0];
        let y = coords[base + 1] as f64 + cras[1];
        let z = coords[base + 2] as f64 + cras[2];
        let q = chain.map_point([x, y, z]);
        coords[base] = q[0] as f32;
        coords[base + 1] = q[1] as f32;
        coords[base + 2] = q[2] as f32;
    }

    if had_cras {
        zero_cras_meta(pointset);
    }

    // Reset every coordsys on the POINTSET array: the original
    // DataSpace -> TransformedSpace matrix no longer describes a valid
    // mapping after an external transform has been applied. Leave
    // DataSpace alone (it still names the input coordinate frame).
    for cs in pointset.coordsys.iter_mut() {
        cs.transformed_space = "NIFTI_XFORM_UNKNOWN".to_string();
        cs.xform = CoordSystem::identity();
    }

    Ok(())
}
