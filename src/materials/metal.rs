use std::{collections::HashMap, f64};

use cgmath::{ElementWise, InnerSpace, Zero};
use log::warn;
use tinyjson::JsonValue;

use crate::{
    json::{json_to_bool, json_to_f64},
    materials::{SampledDirection, roughness_to_exponent},
    samplers::{
        pdf_cosine_hemisphere_power, pdf_inverse_cosine_hemisphere_power,
        sample_cosine_hemisphere_power, sample_inverse_cosine_hemisphere_power,
    },
    texture::{Texture, json_to_texture},
    vec::{Color3, Frame, Point3, Vec2, Vec3, reflect},
};

use super::Material;

pub struct Metal {
    ks: Texture<Color3>,
    exponent: Option<f64>,
    use_fresnel: bool,
    use_blinn: bool,
    sheen: bool,
    normal_map: Option<Texture<Vec3>>,
}

impl Metal {
    #[must_use]
    pub const fn new_with_exponent(
        ks: Texture<Color3>,
        exponent: f64,
        use_fresnel: bool,
        use_blinn: bool,
        sheen: bool,
    ) -> Self {
        Self {
            ks,
            exponent: Some(if sheen { 1.0 / exponent } else { exponent }),
            use_fresnel,
            use_blinn,
            sheen,
            normal_map: None,
        }
    }

    #[must_use]
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        let ks = json_to_texture(json, "ks", Vec3::new(1.0, 1.0, 1.0));
        let mut roughness = json_to_f64(json, "roughness", 0.0);
        if roughness >= 1.0 {
            warn!("Roughness is above 1.0 ({roughness}), clamping it");
            roughness = 1.0;
        }
        let use_fresnel = json_to_bool(json, "use_fresnel", false);
        let use_blinn = json_to_bool(json, "use_blinn", false);
        let sheen = json_to_bool(json, "sheen", false);
        let normal_map = super::json_to_normal_map(json);
        Self {
            ks,
            exponent: roughness_to_exponent(if sheen { 1.0 - roughness } else { roughness }),
            use_fresnel,
            use_blinn,
            sheen,
            normal_map,
        }
    }
}

#[allow(clippy::option_if_let_else)]
impl Material for Metal {
    fn sample(&self, wo: &Vec3, uv: &Vec2, p: &Point3, s: &Vec2) -> Option<SampledDirection> {
        if wo.z < 0.0 {
            return None;
        }

        let weight = if self.use_fresnel {
            self.ks.get(uv, p) - self.ks.get(uv, p).sub_element_wise(1.0) * (1.0 - wo.z).powi(5)
        } else {
            self.ks.get(uv, p)
        };

        let sample_fn = if self.sheen {
            sample_inverse_cosine_hemisphere_power
        } else {
            sample_cosine_hemisphere_power
        };

        if let Some(exponent) = self.exponent {
            let wi = if self.use_blinn {
                let wh = sample_fn(s, exponent);
                reflect(wo, &wh)
            } else {
                let wi_specular = Vec3::new(-wo.x, -wo.y, wo.z);
                let frame = Frame::new(&wi_specular);
                frame.to_world(&sample_fn(s, exponent))
            };
            if wi.z > 0.0 {
                Some(SampledDirection { weight, wi })
            } else {
                None
            }
        } else {
            // perfect reflection
            let wi = Vec3::new(-wo.x, -wo.y, wo.z);
            Some(SampledDirection { weight, wi })
        }
    }

    fn evaluate(&self, wo: &Vec3, wi: &Vec3, uv: &Vec2, p: &Point3) -> Color3 {
        if self.have_delta() || wi.z < 0.0 {
            Color3::zero()
        } else {
            self.ks.get(uv, p) * self.pdf(wo, wi, uv, p)
        }
    }

    fn pdf(&self, wo: &Vec3, wi: &Vec3, _uv: &Vec2, _p: &Point3) -> f64 {
        if self.have_delta() || wi.z < 0.0 {
            0.0
        } else {
            let pdf_fn = if self.sheen {
                pdf_inverse_cosine_hemisphere_power
            } else {
                pdf_cosine_hemisphere_power
            };
            if self.use_blinn {
                let wh = (wo + wi).normalize();
                pdf_fn(&wh, self.exponent.unwrap()) * 0.25 / wi.dot(wh).abs()
            } else {
                let frame = Frame::new(&Vec3::new(-wo.x, -wo.y, wo.z));
                pdf_fn(&frame.to_local(wi), self.exponent.unwrap())
            }
        }
    }

    fn have_delta(&self) -> bool {
        self.exponent.is_none()
    }

    fn emission(&self, _: &Vec3, _uv: &Vec2, _p: &Point3) -> Color3 {
        Color3::zero()
    }

    fn have_emission(&self) -> bool {
        false
    }

    fn get_normal_map_value(&self, uv: &Vec2, p: &Point3) -> Vec3 {
        super::get_normal_map_value_helper(self.normal_map.as_ref(), uv, p)
    }

    fn have_normal_map(&self) -> bool {
        self.normal_map.is_some()
    }
}
