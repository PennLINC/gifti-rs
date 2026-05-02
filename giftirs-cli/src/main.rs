use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand};

mod info;
mod transform;

#[derive(Debug, Parser)]
#[command(
    name = "giftirs",
    version,
    about = "Inspect GIFTI surface files and apply ANTs spatial transforms to vertex coordinates.",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print a human-readable summary of a GIFTI file.
    Info {
        /// Path to the input `.gii` file.
        input: PathBuf,
    },
    /// Apply an ANTs/ITK spatial transform to a GIFTI surface (vertex-warp).
    /// Rust counterpart to `antsApplyTransformsToGifti`.
    ///
    /// THE OPPOSITE-NAMED H5 RULE: to warp a surface FROM space A TO space B,
    /// pass `from-B_to-A_xfm.h5` — the same file you would give
    /// `antsApplyTransforms` to warp an image of B onto A's grid. (Same
    /// convention as `antsApplyTransformsToPoints` and `trxrs`.)
    ///
    /// CARTOON BIDS EXAMPLES — given `sub-01`'s paired h5 files:
    ///
    /// • You have `sub-01_hemi-L_pial.surf.gii` (in T1w space) and want it in
    ///   MNI152NLin6Asym → pass `sub-01_from-MNI152NLin6Asym_to-T1w_xfm.h5`
    ///
    /// • You have `sub-01_hemi-L_pial.surf.gii` (in MNI space) and want it in
    ///   T1w → pass `sub-01_from-T1w_to-MNI152NLin6Asym_xfm.h5`
    ///
    /// • You have `sub-01_hemi-R_white.surf.gii` (in fsaverage) and want it
    ///   in T1w → pass `sub-01_from-T1w_to-fsaverage_xfm.h5`
    ///
    /// WHY OPPOSITE-NAMED? Image warping is pull-based: the chain inside
    /// `from-X_to-Y_xfm.h5` internally maps target Y voxels back to source
    /// X coordinates. Applied to a *point*, that same chain sends a Y-point
    /// to an X-point — so warping a vertex A→B needs the chain in
    /// `from-B_to-A_xfm.h5`.
    ///
    /// FREESURFER C_RAS: surfaces from FreeSurfer store a `VolGeomC_R/A/S`
    /// offset in `CoordinateSystemTransformMatrix`. `giftirs` detects it,
    /// bakes it into the warped vertices, and zeroes it in the output so
    /// downstream tools don't double-apply it. (Matches ANTs' behaviour.)
    ///
    /// FULL INVOCATION (warp T1w-space surface into MNI):
    ///   giftirs transform
    ///   -i sub-01_hemi-L_pial.surf.gii
    ///   -o sub-01_space-MNI152NLin6Asym_hemi-L_pial.surf.gii
    ///   -t sub-01_from-MNI152NLin6Asym_to-T1w_xfm.h5
    Transform {
        /// Input GIFTI surface (`.gii`), e.g. `sub-01_hemi-L_pial.surf.gii`.
        #[arg(short = 'i', long = "input")]
        input: PathBuf,
        /// Output GIFTI surface (`.gii`), e.g.
        /// `sub-01_space-MNI152NLin6Asym_hemi-L_pial.surf.gii`.
        #[arg(short = 'o', long = "output")]
        output: PathBuf,
        /// ANTs/ITK transform: `Composite.h5` (warp + affines), Insight
        /// Transform File V1.0 (`.txt`, affine-only), or ITK MATLAB
        /// (`.mat`, affine-only — what ANTs writes for
        /// `*0GenericAffine.mat`). To warp a surface from space A to space
        /// B, pass `from-B_to-A_xfm.h5` — the SAME file you would give
        /// `antsApplyTransforms` to warp an image of B onto A's grid (see
        /// the description above).
        #[arg(short = 't', long = "transform")]
        transform: PathBuf,
        /// Replace the output file if it exists.
        #[arg(long)]
        overwrite: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result: Result<()> = match cli.command {
        Command::Info { input } => info::run(&input),
        Command::Transform {
            input,
            output,
            transform,
            overwrite,
        } => transform::run(&input, &output, &transform, overwrite),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}
