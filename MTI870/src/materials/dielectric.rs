use std::collections::HashMap;

use cgmath::{InnerSpace, Vector3, Zero};
use log::info;
use tinyjson::JsonValue;

use crate::{
    json::{json_to_f64, json_to_vec3},
    vec::{Color3, Vec2, Vec3},
};

use super::{Material, SampledDirection};

pub struct Dielectric {
    ks: Color3,
    eta_ext: f64,
    eta_int: f64,
}

impl Dielectric {
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Dielectric {
        let ks = json_to_vec3(json, "ks", Vec3::new(1.0, 1.0, 1.0));
        let eta_ext = json_to_f64(json, "eta_ext", 1.0); // eta_i dans les diapositives, si le rayon arrive de l'air
        let eta_int = json_to_f64(json, "eta_int", 1.5); // eta_t dans les diapositives, si le rayon arrive de l'air
        Dielectric {
            ks,
            eta_ext,
            eta_int,
        }
    }
}

impl Material for Dielectric {
    fn sample(&self, wo: &Vec3, sample: &Vec2) -> Option<super::SampledDirection> {
        /* TODO:
        Les principales étapes sont :
        1) On copie eta_int et eta_ext dans eta_t et eta_i. On échange eta_i et eta_t si on arrive sous la surface (ext_int = ext_i et eta_t = eta_ext).
        2) On calcule la valeur de sin(theta_i) avec abs(cos(theta_i)).
        3) On calcule sin(theta_t) à partir de sin(theta_i) en utilisant la loi de Snell.
        4) On vérifie que la valeur de sin(theta_t) est inférieure à 1.
            - Si la norme du vecteur est supérieure à 1, on a une réfraction totale.
              Dans ce cas, on génère une direction de réflexion comme pour les matériaux parfaitement spéculaires.
        5) Sinon, on calcule le Fresnel avec la formule de Schlick.
        6) On génère un nombre aléatoire (avec s.next()) entre [0, 1].
            - Si ce nombre est inférieur au Fresnel, on effectue une réflexion spéculaire.
            - Sinon, on effectue une réfraction (voir le cours pour savoir comment calculer ce vecteur).
        */

        // votrecodeici!("Devoir 1: echantillionage materiau dielectrique");
        // None
        let mut n = Vec3::new(0.0, 0.0, 1.0);
        let mut eta_i = self.eta_ext;
        let mut eta_t = self.eta_int;
        // Determine which medium we're in from wo.z
        // wo.z > 0 => we're on the "outside" side (air), transmission goes into the inside (glass)
        if wo.z < 0.0 {
            eta_i = self.eta_int;
            eta_t = self.eta_ext; // inside -> outside
        }

        let eta = eta_i / eta_t;

        let cos_i = wo.z.abs();
        let sin2_i = (1.0 - cos_i * cos_i);
        let sin2_t = eta * eta * sin2_i;

        let r0 = ((eta_t - eta_i) / (eta_t + eta_i)).powi(2);
        let fresnel = r0 + (1.0 - r0) * (1.0 - cos_i).powi(5);

        // Total internal reflection
        if sin2_t >= 1.0 {
            return Some(SampledDirection {
                weight: self.ks,
                wi: Vec3::new(-wo.x, -wo.y, wo.z),
            });
        }

        // Otherwise we can refract
        let mut cos_t = (1.0 - sin2_t).sqrt();
        // Fresnel coin flip
        if sample.x < fresnel {
            // reflection
            Some(SampledDirection {
                weight: self.ks,
                wi: Vec3::new(-wo.x, -wo.y, wo.z),
            })
        } else {
            // refract
            // - Tangential part scales and flips: wi_xy = -eta * wo_xy
            // - z goes to the opposite hemisphere of wo
            let wi_xy = -eta * Vec3::new(wo.x, wo.y, 0.0);
            let wi_z = if wo.z > 0.0 { -cos_t } else { cos_t };
            Some(SampledDirection {
                weight: self.ks,
                wi: Vec3::new(wi_xy.x, wi_xy.y, wi_z),
            })
        }
    }

    fn emission(&self, _: &Vec3) -> Color3 {
        Color3::zero()
    }

    fn have_emission(&self) -> bool {
        false
    }

    fn evaluate(&self, wo: &Vec3, wi: &Vec3) -> Color3 {
        Color3::zero()
    }

    fn pdf(&self, wo: &Vec3, wi: &Vec3) -> f64 {
        0.0
    }

    fn have_delta(&self) -> bool {
        true
    }
}
