# gifti-rs

A pure-Rust reader and writer for [GIFTI](https://www.nitrc.org/projects/gifti/)
surface files (`.gii`), with a `giftirs` command-line tool for inspecting files
and applying ANTs spatial transforms to vertex coordinates.

This is the GIFTI counterpart to [trx-rs](https://github.com/mattcieslak/trx-rs)
and uses [itk-transforms-rs](https://github.com/mattcieslak/itk-transforms-rs)
to apply ANTs `Composite.h5`, Insight `.txt`, or ITK MATLAB `.mat`
transforms in RAS+ mm.

## Library

```rust
use gifti_rs::{read, write};

let mut img = read("lh.pial.surf.gii".as_ref())?;
// ... mutate vertices ...
write(&img, "lh.pial.warped.surf.gii".as_ref())?;
```

The full GIFTI structure is preserved on round-trip: face topology, metadata,
label tables, coordinate-system records, and all non-pointset data arrays.

## CLI

```sh
giftirs info lh.pial.surf.gii
giftirs transform lh.pial.surf.gii lh.pial.warped.surf.gii \
    --transform InverseComposite.h5
```

## Applying ANTs transforms to surfaces

`giftirs transform` is the Rust counterpart to `antsApplyTransformsToGifti`.
It warps GIFTI vertex coordinates through an ITK Composite `.h5` (with
embedded warp + affines), an Insight Transform File V1.0 (`.txt`,
affine-only), or an ITK MATLAB v4 binary (`.mat`, affine-only — what
ANTs writes for `*0GenericAffine.mat`).

### The "opposite-named h5" rule (cartoon BIDS)

Surfaces, like tractograms, warp in the **opposite** spatial direction
from images. With paired BIDS h5 files for subject `sub-01`:

| You have                                          | You want                         | Pass to `--transform`                       |
| ------------------------------------------------- | -------------------------------- | ------------------------------------------- |
| `sub-01_hemi-L_pial.surf.gii` (in T1w space)      | surface in `MNI152NLin6Asym`     | `sub-01_from-MNI152NLin6Asym_to-T1w_xfm.h5` |
| `sub-01_hemi-L_pial.surf.gii` (in MNI space)      | surface in T1w                   | `sub-01_from-T1w_to-MNI152NLin6Asym_xfm.h5` |
| `sub-01_hemi-R_white.surf.gii` (in `fsaverage`)   | surface in T1w                   | `sub-01_from-T1w_to-fsaverage_xfm.h5`       |

If you are coming from `antsApplyTransforms` for images: pass the **same
h5** you would use to warp an image of the destination space *into* the
source space. (Same convention as `antsApplyTransformsToPoints`.)

### Why opposite-named?

Image warping with `antsApplyTransforms` is **pull-based**: the chain
inside `from-X_to-Y_xfm.h5` (the file that warps an X-image onto a Y-grid,
per BIDS) internally maps target Y voxels back to source X coordinates.
Applied to a *point*, that same chain sends a Y-point to an X-point. So to
warp a vertex FROM space A TO space B, you need a chain that maps
A → B — which lives in the opposite-named file `from-B_to-A_xfm.h5`.

### Worked example (warp T1w-space surface into MNI)

```bash
giftirs transform \
    sub-01_hemi-L_pial.surf.gii \
    sub-01_space-MNI152NLin6Asym_hemi-L_pial.surf.gii \
    --transform sub-01_from-MNI152NLin6Asym_to-T1w_xfm.h5
```

### FreeSurfer C_RAS handling

The `VolGeomC_R/A/S` C_RAS offset that FreeSurfer-produced GIFTI files store
in the `CoordinateSystemTransformMatrix` is detected automatically, baked
into the output coordinates, and zeroed out in the output metadata so
downstream tools do not double-apply it. Matches `antsApplyTransformsToGifti`.

## License

Dual-licensed under MIT or Apache-2.0, at your option.
