#![allow(warnings)]

use cgmath::Zero;
use clap::Parser;
use log::{error, info};
use render::{
    Real,
    array2d::Array2d,
    image::{image_load, image_save},
    vec::Color3,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// image a tester
    #[arg(short, long)]
    test: String,

    /// image de reference
    #[arg(short, long)]
    reference: String,

    /// image de sortie
    #[arg(short, long)]
    output: Option<String>,

    /// Multiplier to the error image
    #[arg(short, long, default_value_t = 1.0)]
    multiplier: f64,

    /// Threshold for compare the images (mostly for png images)
    #[arg(short, long, default_value_t = 2.0 / 255.0)]
    threshold: f64,
}

pub fn main() -> render::Result<()> {
    let args = Args::parse();
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    // Load test and reference image
    let test = image_load(&args.test, true)?;
    let reference = image_load(&args.reference, true)?;

    // Check we have similar size
    if test.size_x() != reference.size_x() || test.size_y() != reference.size_y() {
        panic!(
            "Test and reference images have different size ({:?} vs {:?}",
            test.size(),
            reference.size()
        );
    }

    // Compute diff
    let mut diff = Array2d::with_size(test.size_x(), test.size_y(), Color3::zero());
    let mut mad = Color3::zero();
    for x in 0..test.size_x() {
        for y in 0..test.size_y() {
            let d = test.at(x, y) - reference.at(x, y);
            let d = Color3::new(d.x.abs(), d.y.abs(), d.z.abs());
            *diff.at_mut(x, y) = d * args.multiplier;
            mad += d;
        }
    }
    mad /= (test.size_x() * test.size_y()) as Real;
    let scalar_mad = (mad.x + mad.y + mad.z) / 3.0;
    info!("Mean Absolute Difference: {:?}", mad);
    info!("Scalar MAD: {:?}", scalar_mad);

    if let Some(output) = args.output {
        info!("Writing difference image to '{}'.", output);
        image_save(&output, &diff)?;
    }

    if scalar_mad > args.threshold {
        error!(
            "The images do not matches: {} > {}",
            scalar_mad, args.threshold
        );
    }

    Ok(())
}
