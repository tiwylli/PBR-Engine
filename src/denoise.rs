use std::collections::HashMap;

use log::error;
use tinyjson::JsonValue;

use crate::{
    Result,
    array2d::{Array2d, flattened_arr_vec3},
    image::image_save,
    integrators::{Integrator, albedo::AlbedoIntegrator, normal::NormalIntegrator},
    samplers::Sampler,
    scene::Scene,
    vec::{Color3, Vec3},
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DenoiseType {
    NoDenoise,
    Simple,
    Full,
}

impl DenoiseType {
    #[must_use]
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        json.get("denoise").map_or(Self::NoDenoise, |json_value| {
            if let JsonValue::String(str) = json_value {
                match str.as_str() {
                    "simple" => Self::Simple,
                    "full" => Self::Full,
                    _ => Self::NoDenoise,
                }
            } else {
                Self::NoDenoise
            }
        })
    }
}

pub fn denoise_render(
    render: Array2d<Vec3>,
    scene: &Scene,
    sampler: &mut dyn Sampler,
    denoise_type: DenoiseType,
    denoising_artefacts: bool,
    output: &str,
) -> Result<Array2d<Vec3>> {
    let need_auxiliary_buffers = denoise_type == DenoiseType::Full || denoising_artefacts;
    let mut auxiliary_buffers = None;

    if need_auxiliary_buffers {
        let mut normal_integrator = NormalIntegrator::default();
        let normals = normal_integrator.render(scene, sampler);
        let mut albedo_integrator = AlbedoIntegrator::default();
        let albedos = albedo_integrator.render(scene, sampler);

        if denoising_artefacts {
            save_denoising_artefacts(output, &normals, &albedos)?;
        }

        auxiliary_buffers = Some((albedos, normals));
    }

    if denoise_type == DenoiseType::NoDenoise {
        return Ok(render);
    }

    let flat_im = flattened_arr_vec3(&render);

    let device = oidn::Device::new();
    let mut filter = oidn::RayTracing::new(&device);
    filter.hdr(true).image_dimensions(
        scene.camera.resolution.x as usize,
        scene.camera.resolution.y as usize,
    );

    if denoise_type == DenoiseType::Full {
        if let Some((albedos, normals)) = auxiliary_buffers {
            let flat_albedo = flattened_arr_vec3(&albedos);
            let flat_normal = flattened_arr_vec3(&normals)
                .iter()
                .map(|val| (*val).mul_add(2.0, -1.0))
                .collect::<Vec<_>>();

            filter.albedo_normal(&flat_albedo, &flat_normal);
        }
    }

    let mut filtered = vec![0.0; flat_im.len()];
    filter
        .filter(&flat_im, &mut filtered)
        .expect("Error denoising image");

    if let Err(e) = device.get_error() {
        error!("Error denoising image: {}", e.1);
    }

    Ok(Array2d::<Color3>::from_flat(
        scene.camera.resolution.x,
        scene.camera.resolution.y,
        &filtered,
    ))
}

fn save_denoising_artefacts(
    output: &str,
    normals: &Array2d<Vec3>,
    albedos: &Array2d<Vec3>,
) -> Result<()> {
    let path = output.split('.').collect::<Vec<_>>();
    let mut normals_path = path.clone();
    let mut albedos_path = path;
    normals_path.insert(normals_path.len() - 1, "_normals.");
    albedos_path.insert(albedos_path.len() - 1, "_albedos.");
    image_save(&normals_path.join(""), normals)?;
    image_save(&albedos_path.join(""), albedos)?;
    Ok(())
}
