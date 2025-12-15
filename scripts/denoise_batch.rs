#[cfg(not(feature = "oidn"))]
compile_error!("denoise_batch requires the `oidn` feature. Rebuild with `--features oidn`.");

#[cfg(feature = "oidn")]
mod app {
    use std::{
        ffi::OsStr,
        fs,
        path::{Path, PathBuf},
    };

    use clap::Parser;
    use render::{
        Error, Result,
        array2d::{Array2d, flattened_arr_vec3},
        image::{image_load, image_save},
        vec::Vec3,
    };

    /// Batch denoise previously rendered noisy images using saved albedo/normal buffers.
    #[derive(Parser, Debug)]
    #[command(name = "denoise_batch")]
    pub struct Args {
        /// Directory containing noisy images and their *_albedos / *_normals companions.
        pub input_dir: PathBuf,
        /// Where to place denoised outputs. Defaults to <input_dir>/denoised.
        #[arg(short, long)]
        pub output_dir: Option<PathBuf>,
        /// Use color-only denoise even if albedo/normal maps exist.
        #[arg(long)]
        pub simple: bool,
        /// Suffix inserted before the extension for outputs when writing next to inputs.
        #[arg(long, default_value = "_denoised")]
        pub suffix: String,
    }

    pub fn run() -> Result<()> {
        let args = Args::parse();

        let input_dir = args
            .input_dir
            .canonicalize()
            .map_err(to_error("input directory"))?;
        let default_output = input_dir.join("denoised");
        let output_dir = args
            .output_dir
            .clone()
            .unwrap_or_else(|| default_output.clone());
        fs::create_dir_all(&output_dir).map_err(to_error("output directory creation"))?;

        let mut processed = 0_usize;
        for entry in fs::read_dir(&input_dir).map_err(to_error("read input directory"))? {
            let entry = entry.map_err(to_error("read directory entry"))?;
            let path = entry.path();
            if !path.is_file() || is_auxiliary(&path) {
                continue;
            }

            let Some((base, ext)) = split_stem_ext(&path) else {
                continue;
            };
            let albedo_path = companion_path(&path, "_albedos", &ext);
            let normal_path = companion_path(&path, "_normals", &ext);

            if !albedo_path.exists() || !normal_path.exists() {
                eprintln!(
                    "Skipping {} (missing albedo/normal artefacts)",
                    path.display()
                );
                continue;
            }

            let output_path = if args.output_dir.is_some() {
                output_dir.join(path.file_name().unwrap())
            } else {
                output_dir.join(format!("{base}{}.{ext}", args.suffix))
            };

            println!(
                "Denoising {} using {} and {} -> {}",
                path.display(),
                albedo_path.display(),
                normal_path.display(),
                output_path.display()
            );

            denoise_image_set(&path, &albedo_path, &normal_path, &output_path, args.simple)?;
            processed += 1;
        }

        println!("Done. Denoised {} image(s).", processed);
        Ok(())
    }

    fn denoise_image_set(
        color_path: &Path,
        albedo_path: &Path,
        normal_path: &Path,
        output_path: &Path,
        simple: bool,
    ) -> Result<()> {
        let hdr = is_exr(color_path);
        let color = image_load(color_path.to_str().unwrap(), !hdr)?;
        let albedo = image_load(albedo_path.to_str().unwrap(), !hdr)?;
        let normal = image_load(normal_path.to_str().unwrap(), !hdr)?;

        ensure_same_size(&color, &albedo, color_path, albedo_path)?;
        ensure_same_size(&color, &normal, color_path, normal_path)?;

        let device = oidn::Device::new();
        let mut filter = prepare_filter(&device, color.size_x(), color.size_y());

        if !simple {
            let flat_albedo = flattened_arr_vec3(&albedo);
            let flat_normal = flattened_arr_vec3(&normal)
                .iter()
                .map(|val| (*val).mul_add(2.0, -1.0))
                .collect::<Vec<_>>();
            filter.albedo_normal(&flat_albedo, &flat_normal);
        }

        let flat_color = flattened_arr_vec3(&color);
        let mut filtered = vec![0.0; flat_color.len()];
        filter
            .filter(&flat_color, &mut filtered)
            .expect("Error denoising image");

        if let Err(e) = device.get_error() {
            eprintln!(
                "OIDN reported an error while denoising {}: {}",
                color_path.display(),
                e.1
            );
        }

        let denoised = Array2d::<Vec3>::from_flat(color.size_x(), color.size_y(), &filtered);
        image_save(output_path.to_str().unwrap(), &denoised)?;
        Ok(())
    }

    fn prepare_filter(device: &oidn::Device, size_x: u32, size_y: u32) -> oidn::RayTracing<'_> {
        let mut filter = oidn::RayTracing::new(device);
        filter
            .hdr(true)
            .image_dimensions(size_x as usize, size_y as usize);
        filter
    }

    fn is_auxiliary(path: &Path) -> bool {
        let name = path.file_stem().and_then(OsStr::to_str).unwrap_or_default();
        name.ends_with("_normals") || name.ends_with("_albedos")
    }

    fn split_stem_ext(path: &Path) -> Option<(String, String)> {
        let stem = path.file_stem()?.to_string_lossy();
        let ext = path.extension()?.to_string_lossy();
        Some((stem.into_owned(), ext.into_owned()))
    }

    fn companion_path(path: &Path, suffix: &str, ext: &str) -> PathBuf {
        let base = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        path.with_file_name(format!("{base}{suffix}.{ext}"))
    }

    fn ensure_same_size(
        a: &Array2d<Vec3>,
        b: &Array2d<Vec3>,
        a_path: &Path,
        b_path: &Path,
    ) -> Result<()> {
        if a.size_x() != b.size_x() || a.size_y() != b.size_y() {
            return Err(Error::Other(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Size mismatch between {} ({}x{}) and {} ({}x{})",
                    a_path.display(),
                    a.size_x(),
                    a.size_y(),
                    b_path.display(),
                    b.size_x(),
                    b.size_y()
                ),
            ))));
        }
        Ok(())
    }

    fn is_exr(path: &Path) -> bool {
        path.extension()
            .and_then(OsStr::to_str)
            .map_or(false, |ext| ext.eq_ignore_ascii_case("exr"))
    }

    fn to_error(context: &'static str) -> impl Fn(std::io::Error) -> Error {
        move |err| {
            Error::Other(Box::new(std::io::Error::new(
                err.kind(),
                format!("{context}: {err}"),
            )))
        }
    }
}

#[cfg(feature = "oidn")]
fn main() -> render::Result<()> {
    app::run()
}
