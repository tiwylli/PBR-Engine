use super::{Material, SampledDirection};
use crate::json::json_to_vec3;
use crate::vec::{
    luminance, pdf_cosine_hemisphere, pdf_cosine_hemisphere_power, sample_cosine_hemisphere,
    sample_cosine_hemisphere_power, Color3, Frame, Vec2, Vec3,
};
use cgmath::InnerSpace;
use std::collections::HashMap;
use std::f64::consts::PI;
use tinyjson::JsonValue;

//ChadGPT idea to make an enum for blinn
// Chadgpt a aidé au refactoring pour utiliser l'enum intelligemment
#[derive(Copy, Clone)]
enum SpecModel {
    Phong,
    Blinn,
}

/// Matériau Phong/Blinn-Phong : mixture d'un lobe diffus et d'un lobe spéculaire
pub struct Phong {
    kd: Color3,       // Couleur diffuse
    ks: Color3,       // Couleur spéculaire
    exponent: f64,    // Exposant (s)
    prob_spec: f64,   // Probabilité de choisir le lobe spéculaire
    model: SpecModel, // Phong ou Blinn-Phong
}

impl Phong {
    /// Construction à partir d'un JSON
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Phong {
        let kd = json_to_vec3(json, "kd", Color3::new(0.2, 0.2, 0.2));
        let ks = json_to_vec3(json, "ks", Color3::new(0.8, 0.8, 0.8));

        let mut exponent = if let Some(JsonValue::Number(n)) = json.get("exponent") {
            *n as f64
        } else {
            30.0
        };
        if exponent < 1.0 {
            exponent = 1.0;
        }

        // p(spec) = lum(ks) / (lum(ks) + lum(kd))
        let lks = luminance(&ks);
        let lkd = luminance(&kd);
        let denom = lks + lkd;
        let prob_spec = if denom > 0.0 {
            (lks / denom).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // Flag JSON: "model": "phong" | "blinn"  (défaut: phong)
        let model = if let Some(JsonValue::String(s)) = json.get("model") {
            if s.eq_ignore_ascii_case("blinn") || s.eq_ignore_ascii_case("blinn-phong") {
                SpecModel::Blinn
            } else {
                SpecModel::Phong
            }
        } else {
            SpecModel::Phong
        };

        Phong {
            kd,
            ks,
            exponent,
            prob_spec,
            model,
        }
    }

    fn eval_spec(&self, wo: &Vec3, wi: &Vec3) -> Color3 {
        match self.model {
            SpecModel::Phong => {
                // r = direction de réflexion de -wo autour de n=(0,0,1)
                let r = Vec3::new(-wo.x, -wo.y, wo.z).normalize();
                let ndotr = r.dot(*wi).max(0.0);
                // Normalisation Phong: (s+2)/(2π) * (r·wi)^s
                self.ks * ((self.exponent + 2.0) / (2.0 * PI)) * ndotr.powf(self.exponent)
            }
            SpecModel::Blinn => {
                // h = normalize(wo + wi)
                let sum = *wo + *wi;
                if sum.magnitude2() == 0.0 {
                    return Color3::new(0.0, 0.0, 0.0);
                }
                let h = sum.normalize();
                let ndoth = h.z.max(0.0); // n=(0,0,1)
                                          // Normalisation Blinn-Phong classique: (s+8)/(8π) * (n·h)^s
                self.ks * ((self.exponent + 8.0) / (8.0 * PI)) * ndoth.powf(self.exponent)
            }
        }
    }

    fn pdf_spec(&self, wo: &Vec3, wi: &Vec3) -> f64 {
        match self.model {
            SpecModel::Phong => {
                let r = Vec3::new(-wo.x, -wo.y, wo.z).normalize();
                let ndotr = r.dot(*wi).max(0.0);
                // PDF de l'échantillonnage Phong autour de r
                (self.exponent + 1.0) * ndotr.powf(self.exponent) / (2.0 * PI)
            }
            SpecModel::Blinn => {
                // On a échantillonné le half-vector h autour de n via power-cosine
                // p(h) = (s+1)/(2π) * (n·h)^s
                // p(wi) = p(h) / (4 * |wo·h|)
                let sum = *wo + *wi;
                if sum.magnitude2() == 0.0 {
                    return 0.0;
                }
                let h = sum.normalize();
                let ndoth = h.z.max(0.0);
                let wodoth = wo.dot(h).abs().max(1e-12);
                let pdf_h = (self.exponent + 1.0) * ndoth.powf(self.exponent) / (2.0 * PI);
                pdf_h / (4.0 * wodoth)
            }
        }
    }
}

impl Material for Phong {
    fn sample(&self, wo: &Vec3, sample: &Vec2) -> Option<SampledDirection> {
        if wo.z <= 0.0 {
            return None; // below the surface
        }

        // Choix hiérarchique du lobe
        let mut xi = *sample;
        let wi: Vec3;

        if self.prob_spec > 0.0 && xi.x < self.prob_spec {
            // Lobe spéculaire
            xi.x /= self.prob_spec;

            match self.model {
                SpecModel::Phong => {
                    // Échantillonne autour de r avec power-cosine
                    let s_local = sample_cosine_hemisphere_power(&xi, self.exponent);
                    let r = Vec3::new(-wo.x, -wo.y, wo.z).normalize();
                    let frame = Frame::new(&r);
                    let cand = frame.to_world(&s_local);
                    if cand.z <= 0.0 {
                        return None;
                    }
                    wi = cand;
                }
                SpecModel::Blinn => {
                    // Échantillonne le half-vector h autour de n (Z+)
                    let h = sample_cosine_hemisphere_power(&xi, self.exponent);
                    // Réflexion de wo autour de h: wi = 2(wo·h)h - wo
                    let wodoth = wo.dot(h);
                    let cand = (h * (2.0 * wodoth)) - *wo;
                    if cand.z <= 0.0 {
                        return None;
                    }
                    wi = cand.normalize();
                }
            }
        } else {
            // Lobe diffus (cosine hemisphere)
            let denom = (1.0 - self.prob_spec).max(1e-12);
            xi.x = (xi.x - self.prob_spec).max(0.0) / denom;
            wi = sample_cosine_hemisphere(&xi);
        }

        // Mixture PDF et BSDF*cos via les méthodes evaluate/pdf
        // chadgpt added this when i was looking for problems of phong
        // Adrien m'a ensuite dit de l'ajouter dans blend et fresnel blend
        let pdf = self.pdf(wo, &wi);
        if pdf <= 0.0 {
            return None;
        }
        let fcos = self.evaluate(wo, &wi);
        Some(SampledDirection {
            wi,
            weight: fcos / pdf,
        })
    }

    fn evaluate(&self, wo: &Vec3, wi: &Vec3) -> Color3 {
        if wi.z <= 0.0 {
            return Color3::new(0.0, 0.0, 0.0);
        }
        let cos_theta = wi.z.max(0.0);

        // Diffus Lambert: kd/π
        let eval_diffuse = self.kd / PI;

        // Spéculaire selon le modèle
        let eval_spec = self.eval_spec(wo, wi);

        // BSDF * cos(theta)
        (eval_diffuse + eval_spec) * cos_theta
    }

    fn pdf(&self, wo: &Vec3, wi: &Vec3) -> f64 {
        if wi.z <= 0.0 {
            return 0.0;
        }
        let pdf_spec = self.pdf_spec(wo, wi);
        let pdf_diff = pdf_cosine_hemisphere(wi);
        self.prob_spec * pdf_spec + (1.0 - self.prob_spec) * pdf_diff
    }

    fn have_delta(&self) -> bool {
        // Exposant fini -> non-delta
        false
    }

    fn emission(&self, _wo: &Vec3) -> Color3 {
        Color3::new(0.0, 0.0, 0.0)
    }

    fn have_emission(&self) -> bool {
        false
    }
}
