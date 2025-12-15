use std::time::Instant;

// Lecture des entree ligne de commande
use clap::Parser;
use log::{error, info};
use render::{
    Result,
    fileresolver::FILE_RESOLVER,
    image::image_save,
    json::merge_json,
    scene::{Scene, create_example_scene},
};
use tinyjson::JsonValue;

#[macro_use]
extern crate scan_fmt;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// fichier de scene or `example_scene%d`
    #[arg(short, long, default_value = "example_scene1")]
    input: String,

    /// fichier de scene or `example_scene%d`
    #[arg(short, long)]
    additional: Option<String>,

    /// inline json
    #[arg(short = 'A', long)]
    additional_inline: Option<String>,

    /// image de sortie
    #[arg(short, long, default_value = "out.png")]
    output: String,

    /// nombre d'echantilions
    #[arg(short, long, default_value_t = -1)]
    nspp: i32,

    /// Log ouput
    #[arg(short, long)]
    log: Option<String>,

    /// Number of threads (0 = all cores, -N = all cores - N, N = number of threads)
    #[arg(short, long, default_value_t = 0, allow_hyphen_values = true)]
    threads: i32,

    /// Factor for image resolution
    #[arg(short, long, default_value_t = 1.0)]
    scale: f32,
}

#[allow(clippy::significant_drop_tightening)]
fn main() -> Result<()> {
    // Lecture de la ligne de commande
    let args = Args::parse();
    if let Some(log_out) = args.log {
        let target = Box::new(std::fs::File::create(log_out).expect("Can't create file"));
        pretty_env_logger::formatted_builder()
            .filter_level(log::LevelFilter::Info)
            .target(env_logger::Target::Pipe(target))
            .init();
    } else {
        pretty_env_logger::formatted_builder()
            .filter_level(log::LevelFilter::Info)
            .init();
    }

    // Set number of threads
    if args.threads != 0 {
        let nbthreads = if args.threads < 0 {
            #[allow(clippy::cast_possible_wrap)]
            (num_cpus::get() as i32 - args.threads).max(1)
        } else {
            args.threads
        };
        info!("Number threads set at : {nbthreads}");
        rayon::ThreadPoolBuilder::new()
            .num_threads(nbthreads as usize)
            .build_global()
            .unwrap();
    }

    // Load scene
    let start = Instant::now();
    // 1) Load main file
    let mut json: JsonValue = if let Some(id) = scan_fmt_some!(&args.input, "example_scene{d}", u32)
    {
        create_example_scene(id)
    } else {
        FILE_RESOLVER
            .lock()
            .unwrap()
            .append(std::path::Path::new(&args.input).parent().unwrap());
        let contents =
            std::fs::read_to_string(&args.input).expect("Impossible le fichier de scene");
        contents
            .parse()
            .map_err(|err| render::Error::Other(Box::new(err)))?
    };
    let json = json.get_mut().unwrap();
    // 2) Load additional file and apply patch
    if let Some(add) = args.additional {
        FILE_RESOLVER
            .lock()
            .unwrap()
            .append(std::path::Path::new(&add).parent().unwrap());
        let contents = std::fs::read_to_string(add).expect("Impossible le fichier additionel");
        let add: JsonValue = contents
            .parse()
            .map_err(|err| render::Error::Other(Box::new(err)))?;
        merge_json(json, add.get().unwrap())?;
    }
    if let Some(add) = args.additional_inline {
        let add: JsonValue = add
            .parse()
            .map_err(|err| render::Error::Other(Box::new(err)))?;
        merge_json(json, add.get().unwrap())?;
    }

    // Add additional paths (to facilirate object loading)
    if json.contains_key("paths") {
        let scene_dir = std::path::Path::new(&args.input).parent().unwrap();
        let paths: &Vec<JsonValue> = json["paths"].get().unwrap();
        for p in paths {
            let p_str: &String = p.get().unwrap();
            FILE_RESOLVER
                .lock()
                .unwrap()
                .append(&scene_dir.join(std::path::Path::new(&p_str)));
        }
    }

    // Show information paths
    {
        let locked_resolver = FILE_RESOLVER.lock().unwrap();
        let paths = locked_resolver.paths();
        for p in paths {
            info!(" - {}", p.display());
        }
    }

    let (mut scene, mut sampler, mut int) = Scene::from_json(json);
    info!("Load scene time: {:?}", start.elapsed());

    #[cfg(feature = "oidn")]
    let denoise = {
        let denoise = render::denoise::DenoiseType::from_json(json);
        info!("Denoising: {denoise:?}");
        denoise
    };

    // Scale image resolution
    #[allow(clippy::float_cmp)]
    if args.scale != 1.0 {
        info!("Scale image resolution by factor: {}", args.scale);
        scene.camera.scale(args.scale);
    }

    // Rendering
    let start = Instant::now();
    if args.nspp > 0 {
        info!("Change SPP to: {}", args.nspp);
        sampler.set_nb_samples(args.nspp as usize);
    }
    let render = int.render(&scene, sampler.as_mut());
    if !render.is_finite() {
        error!("Image contains INFs/NaNs");
    }
    info!("Rendering time: {:?}", start.elapsed());

    #[cfg(feature = "oidn")]
    let denoising_artefacts = json
        .get("denoising_artefacts")
        .is_some_and(|value| match value {
            JsonValue::Boolean(b) => *b,
            _ => false,
        });

    #[cfg(not(feature = "oidn"))]
    let img_to_save = render;

    #[cfg(feature = "oidn")]
    let img_to_save = render::denoise::denoise_render(
        render,
        &scene,
        sampler.as_mut(),
        denoise,
        denoising_artefacts,
        &args.output,
    )?;

    image_save(&args.output, &img_to_save)?;

    Ok(())
}
