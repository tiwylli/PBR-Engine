use std::{collections::HashMap, f64::consts::PI};

use crate::{
    materials::SampledDirection,
    vec::{Frame, Vec2},
};
use cgmath::{InnerSpace, Vector3, Zero};
use log::{info, warn};
use rand::distr::weighted::{self, Weight};
use tinyjson::JsonValue;

use crate::{
    json::{json_to_f64, json_to_vec3},
    vec::{pdf_cosine_hemisphere_power, sample_cosine_hemisphere_power, Color3, Vec3},
};

use super::{random_in_unit_sphere, Material};

pub struct Metal {
    ks: Color3,
    roughness: f64,
    use_fresnel: bool,
    eta: Option<Color3>, // n (RGB)
    k: Option<Color3>,   // kappa (RGB)
}

impl Metal {
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Metal {
        let ks = json_to_vec3(json, "ks", Vec3::new(1.0, 1.0, 1.0));
        let mut roughness = json_to_f64(json, "roughness", 0.0);
        if roughness >= 1.0 {
            warn!("Roughness is above 1.0 ({}), clamping it", roughness);
            roughness = 1.0;
        }
        // options Fresnel conducteur ---
        let use_fresnel = match json.get("use_fresnel") {
            Some(JsonValue::Boolean(b)) => *b,
            _ => false,
        };

        let (eta, k) = if use_fresnel {
            let eta = json
                .get("eta")
                .unwrap_or_else(|| panic!("Missing 'eta' for Metal with use_fresnel=true"));
            let k = json
                .get("k")
                .unwrap_or_else(|| panic!("Missing 'k' for Metal with use_fresnel=true"));
            (
                Some(json_to_vec3(json, "eta", Vec3::new(0.0, 0.0, 0.0))),
                Some(json_to_vec3(json, "k", Vec3::new(0.0, 0.0, 0.0))),
            )
        } else {
            (None, None)
        };
        Metal {
            ks,
            roughness,
            use_fresnel,
            eta,
            k,
        }
    }

    // Fresnel pour conducteur, par canal --- with the help of my boy chadgpt
    fn fresnel_conductor(&self, cos_i: f64) -> Color3 {
        // clamp cos_i pour stabilité numérique
        let c = cos_i.clamp(0.0, 1.0);

        let (ex, ey, ez) = (
            self.eta.unwrap().x,
            self.eta.unwrap().y,
            self.eta.unwrap().z,
        );
        let (kx, ky, kz) = (self.k.unwrap().x, self.k.unwrap().y, self.k.unwrap().z);

        // helper canal
        fn f_channel(eta: f64, k: f64, c: f64) -> f64 {
            let c2 = c * c;
            let eta2_k2 = eta * eta + k * k;

            let two_eta_c = 2.0 * eta * c;

            let r_par_n = eta2_k2 * c2 - two_eta_c + 1.0;
            let r_par_d = eta2_k2 * c2 + two_eta_c + 1.0;

            let r_perp_n = eta2_k2 - two_eta_c + c2;
            let r_perp_d = eta2_k2 + two_eta_c + c2;

            0.5 * ((r_par_n / r_par_d) + (r_perp_n / r_perp_d))
        }

        Color3::new(
            f_channel(ex, kx, c),
            f_channel(ey, ky, c),
            f_channel(ez, kz, c),
        )
    }

    fn apply_fresnel(&self, cos_i: f64) -> Color3 {
        if self.use_fresnel {
            self.fresnel_conductor(cos_i)
        } else {
            Color3::new(1.0, 1.0, 1.0)
        }
    }

    fn phong_exponent(&self) -> f64 {
        if self.roughness <= 0.0 {
            0.0
        } else {
            (2.0 / (self.roughness * self.roughness)).max(0.0) - 2.0
        }
    }
}

impl Material for Metal {
    fn sample(&self, wo: &Vec3, sample: &Vec2) -> Option<super::SampledDirection> {
        // Rayon arrivant sous la surface -> pas d'échantillon
        if wo.z < 0.0 {
            return None;
        }
        let mut weight;
        if self.use_fresnel {
            //info!("use fresnel");
            // Fresnel (conducteur) évalué à l'angle incident
            let fresnel = self.apply_fresnel(wo.z.abs());
            // Poids = ks .* Fresnel (par canal)
            weight = Color3::new(
                self.ks.x * fresnel.x,
                self.ks.y * fresnel.y,
                self.ks.z * fresnel.z,
            );
        } else {
            //info!("no fresnel");
            weight = self.ks;
        }

        if self.roughness == 0.0 {
            // réflexion spéculaire parfaite
            let wi = Vector3::new(-wo.x, -wo.y, wo.z);
            return Some(SampledDirection { weight, wi });
        }
        // slides  cours 5 page 59
        let n = self.phong_exponent();
        let s_local = sample_cosine_hemisphere_power(sample, n);
        let r: Vector3<f64> = Vec3::new(-wo.x, -wo.y, wo.z).normalize();
        let frame = Frame::new(&r);
        let wi = frame.to_world(&s_local);
        if wi.z <= 0.0 {
            return None;
        }
        return Some(SampledDirection { weight, wi });

        //  else {
        //     // réflexion "fuzzy" : spéculaire perturbée
        //     let wperf = Vector3::new(-wo.x, -wo.y, wo.z);
        //     let fuzz = self.roughness * random_in_unit_sphere(sample.x);
        //     let wi = (wperf + fuzz).normalize();
        //     if wi.z < 0.0 {
        //         return None;
        //     }
        //     return Some(SampledDirection { weight, wi });
        // }
    }

    fn emission(&self, _: &Vec3) -> Color3 {
        Color3::zero()
    }

    fn have_emission(&self) -> bool {
        false
    }

    fn evaluate(&self, wo: &Vec3, wi: &Vec3) -> Color3 {
        if self.roughness == 0.0 {
            return Color3::zero();
        }
        if wo.z <= 0.0 || wi.z <= 0.0 {
            return Color3::zero();
        }
        //Lobe Phong
        let r = Vec3::new(-wo.x, -wo.y, wo.z).normalize();
        let ndotr = r.dot(*wi).max(0.0);
        let n = self.phong_exponent();

        let lobe = (n + 1.0) * ndotr.powf(n) / (2.0 * PI);
        let fr_scale = if self.use_fresnel {
            let f = self.apply_fresnel(wo.z.abs());
            Color3::new(self.ks.x * f.x, self.ks.y * f.y, self.ks.z * f.z)
        } else {
            self.ks
        };
        // Correction: voir sujet
        return fr_scale * self.pdf(wo, wi);
    }

    fn pdf(&self, wo: &Vec3, wi: &Vec3) -> f64 {
        if self.roughness == 0.0 {
            return 0.0;
        }
        if wo.z <= 0.0 || wi.z <= 0.0 {
            return 0.0;
        }
        let r = Vec3::new(-wo.x, -wo.y, wo.z).normalize();
        let ndotr = r.dot(*wi).max(0.0);
        let n = self.phong_exponent();
        let lobe = (n + 1.0) * ndotr.powf(n) / (2.0 * PI);
        return lobe;
    }

    fn have_delta(&self) -> bool {
        return self.roughness == 0.0;
    }
}
