use std::{collections::HashMap, f64};

use cgmath::{Array, InnerSpace, Zero};
use tinyjson::JsonValue;

use crate::{
    Real,
    json::json_to_f64,
    materials::{Material, SampledDirection},
    texture::{Texture, json_to_texture, json_to_texture_float},
    vec::{Color3, Frame, Point3, Vec2, Vec3, luminance},
};

pub struct PrincipledBsdf {
    base_color: Texture<Color3>,
    specular_transmission: Texture<Real>,
    metallic: Texture<Real>,
    subsurface: Texture<Real>,
    specular: Texture<Real>,
    roughness: Texture<Real>,
    specular_tint: Texture<Real>,
    anisotropic: Texture<Real>,
    sheen: Texture<Real>,
    sheen_tint: Texture<Real>,
    clearcoat: Texture<Real>,
    clearcoat_gloss: Texture<Real>,
    eta: Real,
    normal_map: Option<Texture<Vec3>>,
}

impl PrincipledBsdf {
    #[must_use]
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        let base_color = json_to_texture(json, "base_color", Vec3::from_value(0.8));
        let specular_transmission = json_to_texture_float(json, "specular_transmission", 0.0);
        let metallic = json_to_texture_float(json, "metallic", 0.0);
        let subsurface = json_to_texture_float(json, "subsurface", 0.0);
        let specular = json_to_texture_float(json, "specular", 0.5);
        let roughness = json_to_texture_float(json, "roughness", 0.25);
        let specular_tint = json_to_texture_float(json, "specular_tint", 0.0);
        let anisotropic = json_to_texture_float(json, "anisotropic", 0.0);
        let sheen = json_to_texture_float(json, "sheen", 0.0);
        let sheen_tint = json_to_texture_float(json, "sheen_tint", 0.0);
        let clearcoat = json_to_texture_float(json, "clearcoat", 0.0);
        let clearcoat_gloss = json_to_texture_float(json, "clearcoat_gloss", 0.0);
        let eta = json_to_f64(json, "eta", 1.5);
        let normal_map = super::json_to_normal_map(json);

        Self {
            base_color,
            specular_transmission,
            metallic,
            subsurface,
            specular,
            roughness,
            specular_tint,
            anisotropic,
            sheen,
            sheen_tint,
            clearcoat,
            clearcoat_gloss,
            eta,
            normal_map,
        }
    }
}

impl Material for PrincipledBsdf {
    fn sample(&self, wo: &Vec3, uv: &Vec2, p: &Point3, s: &Vec2) -> Option<SampledDirection> {
        let pbsdf_sample = PBsdfSample::new(self, uv, p);

        disney_sample(&pbsdf_sample, wo, s)
    }

    fn evaluate(&self, wo: &Vec3, wi: &Vec3, uv: &Vec2, p: &Point3) -> Color3 {
        let pbsdf_sample = PBsdfSample::new(self, uv, p);
        let h = (wo + wi).normalize();

        f_disney(&pbsdf_sample, wo, wi, &h)
    }

    #[allow(clippy::suboptimal_flops)]
    fn pdf(&self, wo: &Vec3, wi: &Vec3, uv: &Vec2, p: &Point3) -> f64 {
        let pbsdf_sample = PBsdfSample::new(self, uv, p);
        let diffuse_weight =
            (1.0 - pbsdf_sample.metallic) * (1.0 - pbsdf_sample.specular_transmission);
        let metal_weight = 1.0 - pbsdf_sample.specular_transmission * (1.0 - pbsdf_sample.metallic);
        let glass_weight = (1.0 - pbsdf_sample.metallic) * pbsdf_sample.specular_transmission;
        let clearcoat_weight = 0.25 * pbsdf_sample.clearcoat;

        let total = diffuse_weight + metal_weight + glass_weight + clearcoat_weight;

        let h = (wo + wi).normalize();
        let dm = ggx(&pbsdf_sample, &h);
        let dc = dc(&pbsdf_sample, &h);
        let g_wo = smith_masking_gtr2(wo, pbsdf_sample.alpha_x, pbsdf_sample.alpha_y);

        (diffuse_weight * diffuse_pdf(wi)
            + metal_weight * metal_pdf(wo, &h, dm, g_wo)
            + glass_weight * glass_pdf(&pbsdf_sample, wo, wi)
            + clearcoat_weight * clearcoat_pdf(wi, &h, dc))
            / total
    }

    fn have_delta(&self) -> bool {
        false
    }

    fn emission(&self, _wo: &Vec3, _uv: &Vec2, _p: &Point3) -> Color3 {
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

    fn get_albedo(&self, uv: &Vec2, p: &Point3) -> Color3 {
        self.base_color.get(uv, p)
    }
}

struct PBsdfSample {
    base_color: Color3,
    specular_transmission: Real,
    metallic: Real,
    subsurface: Real,
    specular: Real,
    roughness: Real,
    specular_tint: Real,
    sheen: Real,
    sheen_tint: Real,
    clearcoat: Real,
    clearcoat_gloss: Real,
    eta: Real,
    alpha_x: Real,
    alpha_y: Real,
    c_tint: Color3,
}

impl PBsdfSample {
    fn new(p_bsdf: &PrincipledBsdf, uv: &Vec2, p: &Point3) -> Self {
        let base_color = p_bsdf.base_color.get(uv, p);

        let anisotropic = p_bsdf.anisotropic.get(uv, p);
        let roughness = p_bsdf.roughness.get(uv, p).max(0.01);

        let aspect = 0.9f64.mul_add(-anisotropic, 1.0).sqrt();
        let roughness_2 = roughness * roughness;
        let alpha_x = (roughness_2 / aspect).max(0.0001);
        let alpha_y = (roughness_2 * aspect).max(0.0001);

        let lum = luminance(&base_color);
        let c_tint = if lum > 0.0 {
            base_color / lum
        } else {
            Color3::from_value(1.0)
        };

        Self {
            base_color,
            specular_transmission: p_bsdf.specular_transmission.get(uv, p),
            metallic: p_bsdf.metallic.get(uv, p),
            subsurface: p_bsdf.subsurface.get(uv, p),
            specular: p_bsdf.specular.get(uv, p),
            roughness,
            specular_tint: p_bsdf.specular_tint.get(uv, p),
            sheen: p_bsdf.sheen.get(uv, p),
            sheen_tint: p_bsdf.sheen_tint.get(uv, p),
            clearcoat: p_bsdf.clearcoat.get(uv, p),
            clearcoat_gloss: p_bsdf.clearcoat_gloss.get(uv, p),
            eta: p_bsdf.eta,
            alpha_x,
            alpha_y,
            c_tint,
        }
    }
}

fn diffuse_sample(pbsdf: &PBsdfSample, wo: &Vec3, s: &Vec2) -> Option<SampledDirection> {
    if wo.z < 0.0 {
        return None;
    }

    let wi = crate::samplers::sample_cosine_hemisphere(s);
    let h = (wo + wi).normalize();
    let f_diffuse = f_diffuse(pbsdf, wo, &wi, &h);

    Some(SampledDirection {
        weight: f_diffuse / diffuse_pdf(&wi),
        wi: wi.normalize(),
    })
}

#[allow(clippy::suboptimal_flops, clippy::similar_names)]
fn f_diffuse(pbsdf: &PBsdfSample, wo: &Vec3, wi: &Vec3, h: &Vec3) -> Color3 {
    let h_dot_wi_2 = h.dot(*wi).powi(2);
    let one_minus_n_dot_wo_5 = (1.0 - wo.z.abs()).powi(5);
    let one_minus_n_dot_wi_5 = (1.0 - wi.z.abs()).powi(5);

    let fd90_minus_1 = 2.0 * pbsdf.roughness * h_dot_wi_2 - 0.5;
    let fd_wo = 1.0 + fd90_minus_1 * one_minus_n_dot_wo_5;
    let fd_wi = 1.0 + fd90_minus_1 * one_minus_n_dot_wi_5;
    let f_base_diffuse = pbsdf.base_color * f64::consts::FRAC_1_PI * fd_wi * fd_wo * wi.z.abs();

    let fss90_minus_1 = pbsdf.roughness * h_dot_wi_2 - 1.0;
    let fss_wo = 1.0 + fss90_minus_1 * one_minus_n_dot_wo_5;
    let fss_wi = 1.0 + fss90_minus_1 * one_minus_n_dot_wi_5;
    let inner_term = fss_wo * fss_wi * (1.0 / (wo.z.abs() + wi.z.abs()) - 0.5) + 0.5;
    let f_subsurface = 1.25 * pbsdf.base_color * f64::consts::FRAC_1_PI * inner_term * wi.z.abs();

    (1.0 - pbsdf.subsurface) * f_base_diffuse + pbsdf.subsurface * f_subsurface
}

fn diffuse_pdf(wi: &Vec3) -> f64 {
    crate::samplers::pdf_cosine_hemisphere(wi)
}

fn metal_sample(pbsdf: &PBsdfSample, wo: &Vec3, s: &Vec2) -> Option<SampledDirection> {
    let h = sample_ggx_visible_normal(wo, pbsdf.alpha_x, pbsdf.alpha_y, s);
    let wodoth = wo.dot(h);
    let reflected = (h * (2.0 * wodoth)) - *wo;
    if reflected.z <= 0.0 {
        return None;
    }
    let wi = reflected.normalize();

    let (f_metal, dm, g_wo) = f_metal(pbsdf, wo, &wi, &h);

    Some(SampledDirection {
        weight: f_metal / metal_pdf(wo, dm, g_wo),
        wi,
    })
}

#[allow(clippy::suboptimal_flops, clippy::similar_names)]
fn f_metal(pbsdf: &PBsdfSample, wo: &Vec3, wi: &Vec3, h: &Vec3) -> (Color3, f64, f64) {
    let one_minus_h_dot_wi_5 = (1.0 - h.dot(*wi).abs()).powi(5);

    let ks = Vec3::from_value(1.0 - pbsdf.specular_tint) + pbsdf.specular_tint * pbsdf.c_tint;
    let r0_sqrt = (pbsdf.eta - 1.0) / (pbsdf.eta + 1.0);
    let c0 = pbsdf.specular * r0_sqrt * r0_sqrt * (1.0 - pbsdf.metallic) * ks
        + pbsdf.metallic * pbsdf.base_color;
    let fm = c0 + (Vec3::from_value(1.0) - c0) * one_minus_h_dot_wi_5;
    let dm = ggx(pbsdf, h);

    let g_wo = smith_masking_gtr2(wo, pbsdf.alpha_x, pbsdf.alpha_y);
    let g_wi = smith_masking_gtr2(wi, pbsdf.alpha_x, pbsdf.alpha_y);
    let gm = g_wo * g_wi;

    (fm * dm * gm / (4.0 * wo.z.abs()), dm, g_wo)
}

fn metal_pdf(wo: &Vec3, dm: f64, g_wo: f64) -> f64 {
    dm * g_wo / (4.0 * wo.z.abs())
}

fn clearcoat_sample(pbsdf: &PBsdfSample, wo: &Vec3, s: &Vec2) -> Option<SampledDirection> {
    let alpha_g = (1.0 - pbsdf.clearcoat_gloss).mul_add(0.1, pbsdf.clearcoat_gloss * 0.001);
    let h = sample_clearcoat_normal(alpha_g, s);
    let wodoth = wo.dot(h);
    let reflected = (h * (2.0 * wodoth)) - *wo;
    if reflected.z <= 0.0 {
        return None;
    }
    let wi = reflected.normalize();

    let (f_clearcoat, dc) = f_clearcoat(pbsdf, wo, &wi, &h);
    let pdf = clearcoat_pdf(wo, &wi, &h, dc);

    if pdf > 0.0 {
        Some(SampledDirection {
            weight: f_clearcoat / pdf,
            wi,
        })
    } else {
        None
    }
}

fn sample_clearcoat_normal(alpha: f64, s: &Vec2) -> Vec3 {
    let alpha_2 = alpha * alpha;
    let cos_h_elevation = ((1.0 - alpha_2.powf(1.0 - s.x)) / (1.0 - alpha_2)).sqrt();
    let sin_h_elevation = cos_h_elevation.acos().sin();
    let h_azimuth = 2.0 * f64::consts::PI * s.y;
    Vec3::new(
        sin_h_elevation * h_azimuth.cos(),
        sin_h_elevation * h_azimuth.sin(),
        cos_h_elevation,
    )
}

#[allow(clippy::suboptimal_flops, clippy::similar_names)]
fn f_clearcoat(pbsdf: &PBsdfSample, wo: &Vec3, wi: &Vec3, h: &Vec3) -> (Color3, f64) {
    // R0(eta = 1.5) = (eta - 1)^2 / (eta + 1)^2 = 0.5^2 / 2.5^2 = 0.04
    let one_minus_h_dot_wi_5 = (1.0 - h.dot(*wi).abs()).powi(5);
    let fc = 0.04 + (1.0 - 0.04) * one_minus_h_dot_wi_5;

    let dc = dc(pbsdf, h);

    let lambda_c_wo = lambda_omega(wo, 0.25, 0.25);
    let lambda_c_wi = lambda_omega(wi, 0.25, 0.25);
    let gc_wo = 1.0 / (1.0 + lambda_c_wo);
    let gc_wi = 1.0 / (1.0 + lambda_c_wi);
    let gc = gc_wo * gc_wi;

    (Color3::from_value(fc * dc * gc / (4.0 * wo.z.abs())), dc)
}

fn clearcoat_pdf(wo: &Vec3, wi: &Vec3, h: &Vec3, dc: f64) -> f64 {
    if wo.z < 0.0 || wi.z < 0.0 {
        return 0.0;
    }
    let h_dot_wi = h.dot(*wi);

    dc * h.z.abs() / (4.0 * h_dot_wi.abs())
}

#[allow(dead_code)] // keep for debugging sheen only
fn sheen_sample(pbsdf: &PBsdfSample, wo: &Vec3, s: &Vec2) -> Option<SampledDirection> {
    if wo.z < 0.0 {
        return None;
    }

    let wi = crate::samplers::sample_cosine_hemisphere(s);
    let h = (wo + wi).normalize();
    let f_sheen = f_sheen(pbsdf, &wi, &h);

    Some(SampledDirection {
        weight: f_sheen / sheen_pdf(&wi),
        wi: wi.normalize(),
    })
}

fn f_sheen(pbsdf: &PBsdfSample, wi: &Vec3, h: &Vec3) -> Color3 {
    let c_sheen = Vec3::from_value(1.0 - pbsdf.sheen_tint) + pbsdf.sheen_tint * pbsdf.c_tint;
    let one_minus_h_dot_wi_5 = (1.0 - h.dot(*wi).abs()).powi(5);

    c_sheen * one_minus_h_dot_wi_5 * wi.z.abs()
}

#[allow(dead_code)] // keep for debugging sheen only
fn sheen_pdf(wi: &Vec3) -> f64 {
    crate::samplers::pdf_cosine_hemisphere(wi)
}

#[allow(
    clippy::suboptimal_flops,
    clippy::similar_names,
    clippy::unreadable_literal
)]
fn glass_sample(pbsdf: &PBsdfSample, wo: &Vec3, s: &Vec2) -> Option<SampledDirection> {
    let eta = if wo.z > 0.0 {
        pbsdf.eta
    } else {
        1.0 / pbsdf.eta
    };

    let mut h = sample_ggx_visible_normal(wo, pbsdf.alpha_x, pbsdf.alpha_y, s);
    if h.z < 0.0 {
        h = -h;
    }

    let h_dot_wo = h.dot(*wo);
    let f = fresnel_dielectric2(h_dot_wo, eta);

    let rand = crate::samplers::hash2(*s);
    let wi = if rand < f {
        // reflection
        let reflected = -*wo + 2.0 * h_dot_wo * h;
        if reflected.z <= 0.0 {
            return None;
        }
        reflected.normalize()
    } else {
        // refraction
        let h_dot_wi_2 = 1.0 - (1.0 - h_dot_wo * h_dot_wo) / (eta * eta);
        if h_dot_wo < 0.0 {
            h = -h;
        }
        let h_dot_wi = h_dot_wi_2.sqrt();
        let wi = -*wo / eta + (h_dot_wo.abs() / eta - h_dot_wi) * h;
        if wi.z * wo.z > 0.0 {
            return None;
        }
        wi.normalize()
    };

    let f_glass = f_glass(pbsdf, wo, &wi);
    let pdf = glass_pdf(pbsdf, wo, &wi);
    if !pdf.is_normal() {
        return None;
    }
    Some(SampledDirection {
        weight: f_glass / pdf,
        wi,
    })
}

#[allow(clippy::suboptimal_flops, clippy::similar_names)]
fn f_glass(pbsdf: &PBsdfSample, wo: &Vec3, wi: &Vec3) -> Color3 {
    let reflect = wo.z * wi.z > 0.0;

    let eta = if wo.z > 0.0 {
        pbsdf.eta
    } else {
        1.0 / pbsdf.eta
    };

    let mut h = if reflect {
        (wo + wi).normalize()
    } else {
        (wo + wi * eta).normalize()
    };
    if h.z < 0.0 {
        h = -h;
    }

    let h_dot_wo = h.dot(*wo);
    let h_dot_wi = h.dot(*wi);

    let fg = fresnel_dielectric2(h_dot_wo, eta);

    let dg = ggx(pbsdf, &h);

    let g_wo = smith_masking_gtr2(wo, pbsdf.alpha_x, pbsdf.alpha_y);
    let g_wi = smith_masking_gtr2(wi, pbsdf.alpha_x, pbsdf.alpha_y);
    let gg = g_wo * g_wi;

    if wo.z * wi.z > 0.0 {
        pbsdf.base_color * fg * dg * gg / (4.0 * wo.z.abs())
    } else {
        let base_color_sqrt = Color3::new(
            pbsdf.base_color.x.sqrt(),
            pbsdf.base_color.y.sqrt(),
            pbsdf.base_color.z.sqrt(),
        );

        let term = h_dot_wo + eta * h_dot_wi;
        base_color_sqrt * (1.0 - fg) * dg * gg * (h_dot_wi * h_dot_wo).abs()
            / (wo.z.abs() * term * term)
    }
}

#[allow(clippy::suboptimal_flops, clippy::similar_names)]
fn glass_pdf(pbsdf: &PBsdfSample, wo: &Vec3, wi: &Vec3) -> f64 {
    let reflect = wo.z * wi.z > 0.0;

    let eta = if wo.z > 0.0 {
        pbsdf.eta
    } else {
        1.0 / pbsdf.eta
    };

    let mut h = if reflect {
        (wo + wi).normalize()
    } else {
        (wo + wi * eta).normalize()
    };
    if h.z < 0.0 {
        h = -h;
    }

    let h_dot_wo = h.dot(*wo);

    let f = fresnel_dielectric2(h_dot_wo, eta);
    let d = ggx(pbsdf, &h);
    let g_wo = smith_masking_gtr2(wo, pbsdf.alpha_x, pbsdf.alpha_y);

    if reflect {
        f * d * g_wo / (4.0 * wo.z.abs())
    } else {
        let h_dot_wi = h.dot(*wi);
        let sqrt_denom = h_dot_wo + eta * h_dot_wi;
        let dh_dout = eta * eta * h_dot_wi / (sqrt_denom * sqrt_denom);

        (1.0 - f) * d * g_wo * (dh_dout * h_dot_wo / wo.z).abs()
    }
}

#[allow(clippy::suboptimal_flops)]
fn disney_sample(pbsdf: &PBsdfSample, wo: &Vec3, s: &Vec2) -> Option<SampledDirection> {
    if wo.z < 0.0 {
        return glass_sample(pbsdf, wo, s);
    }

    let mut diffuse_weight = (1.0 - pbsdf.metallic) * (1.0 - pbsdf.specular_transmission);
    let mut metal_weight = 1.0 - pbsdf.specular_transmission * (1.0 - pbsdf.metallic);
    let mut glass_weight = (1.0 - pbsdf.metallic) * pbsdf.specular_transmission;
    let mut clearcoat_weight = 0.25 * pbsdf.clearcoat;

    let total = diffuse_weight + metal_weight + glass_weight + clearcoat_weight;
    diffuse_weight /= total;
    metal_weight /= total;
    glass_weight /= total;
    clearcoat_weight /= total;

    let t1 = diffuse_weight;
    let t2 = diffuse_weight + metal_weight;
    let t3 = t2 + glass_weight;

    let mut s = *s;

    let sd = if s.x < t1 {
        s.x /= t1;
        diffuse_sample(pbsdf, wo, &s)
    } else if s.x < t2 {
        s.x = (s.x - t1) / metal_weight;
        metal_sample(pbsdf, wo, &s)
    } else if s.x < t3 {
        s.x = (s.x - t2) / glass_weight;
        glass_sample(pbsdf, wo, &s)
    } else {
        s.x = (s.x - t3) / clearcoat_weight;
        clearcoat_sample(pbsdf, wo, &s)
    };

    if let Some(mut sd) = sd {
        let h = (wo + sd.wi).normalize();
        let dm = ggx(pbsdf, &h);
        let dc = dc(pbsdf, &h);
        let g_wo = smith_masking_gtr2(wo, pbsdf.alpha_x, pbsdf.alpha_y);

        let pdf = (diffuse_weight * diffuse_pdf(&sd.wi)
            + metal_weight * metal_pdf(wo, dm, g_wo)
            + glass_weight * glass_pdf(pbsdf, wo, &sd.wi)
            + clearcoat_weight * clearcoat_pdf(wo, &sd.wi, &h, dc))
            / total;

        sd.weight = f_disney(pbsdf, wo, &sd.wi, &h) / pdf;
        Some(sd)
    } else {
        sd
    }
}

#[allow(clippy::suboptimal_flops)]
fn f_disney(pbsdf: &PBsdfSample, wo: &Vec3, wi: &Vec3, h: &Vec3) -> Color3 {
    if wo.z < 0.0 {
        return (1.0 - pbsdf.metallic) * pbsdf.specular_transmission * f_glass(pbsdf, wo, wi);
    }

    let f_diffuse = f_diffuse(pbsdf, wo, wi, h);
    let f_sheen = f_sheen(pbsdf, wi, h);
    let f_metal = f_metal(pbsdf, wo, wi, h).0;
    let f_clearcoat = f_clearcoat(pbsdf, wo, wi, h).0;
    let f_glass = f_glass(pbsdf, wo, wi);

    (1.0 - pbsdf.specular_transmission) * (1.0 - pbsdf.metallic) * f_diffuse
        + (1.0 - pbsdf.metallic) * pbsdf.sheen * f_sheen
        + (1.0 - pbsdf.specular_transmission) * pbsdf.metallic * f_metal
        + 0.25 * pbsdf.clearcoat * f_clearcoat
        + (1.0 - pbsdf.metallic) * pbsdf.specular_transmission * f_glass
}

#[inline]
#[allow(clippy::suboptimal_flops)]
fn ggx(pbsdf: &PBsdfSample, h: &Vec3) -> f64 {
    let hx_ax = h.x / pbsdf.alpha_x;
    let hy_ay = h.y / pbsdf.alpha_y;
    let temp = hx_ax * hx_ax + hy_ay * hy_ay + h.z * h.z;
    1.0 / (f64::consts::PI * pbsdf.alpha_x * pbsdf.alpha_y * temp * temp)
}

#[inline]
#[allow(clippy::suboptimal_flops)]
fn lambda_omega(w: &Vec3, alpha_x: f64, alpha_y: f64) -> f64 {
    let tx = w.x * alpha_x;
    let ty = w.y * alpha_y;
    (-1.0 + (1.0 + (tx * tx + ty * ty) / (w.z * w.z)).sqrt()) * 0.5
}

#[inline]
fn smith_masking_gtr2(w: &Vec3, alpha_x: f64, alpha_y: f64) -> f64 {
    1.0 / (1.0 + lambda_omega(w, alpha_x, alpha_y))
}

#[inline]
#[allow(clippy::suboptimal_flops, clippy::similar_names)]
fn fresnel_dielectric3(h_dot_wo: f64, h_dot_wi: f64, eta: f64) -> f64 {
    let rs = (h_dot_wo - eta * h_dot_wi) / (h_dot_wo + eta * h_dot_wi);
    let rp = (eta * h_dot_wo - h_dot_wi) / (eta * h_dot_wo + h_dot_wi);
    (rs * rs + rp * rp) * 0.5
}

#[inline]
#[allow(clippy::suboptimal_flops, clippy::similar_names)]
fn fresnel_dielectric2(h_dot_wo: f64, eta: f64) -> f64 {
    let n_dot_wi_2 = 1.0 - (1.0 - h_dot_wo * h_dot_wo) / (eta * eta);
    if n_dot_wi_2 < 0.0 {
        // total internal reflection
        1.0
    } else {
        let n_dot_wi = n_dot_wi_2.sqrt();
        fresnel_dielectric3(h_dot_wo.abs(), n_dot_wi, eta)
    }
}

#[inline]
fn dc(pbsdf: &PBsdfSample, h: &Vec3) -> f64 {
    let alpha_g = (1.0 - pbsdf.clearcoat_gloss).mul_add(0.1, pbsdf.clearcoat_gloss * 0.001);
    let alpha_g_2 = alpha_g * alpha_g;
    (alpha_g_2 - 1.0)
        / (f64::consts::PI * alpha_g_2.ln() * (alpha_g_2 - 1.0).mul_add(h.z * h.z, 1.0))
}

#[allow(clippy::suboptimal_flops)]
fn sample_ggx_visible_normal(wo: &Vec3, alpha_x: f64, alpha_y: f64, sample: &Vec2) -> Vec3 {
    if wo.z < 0.0 {
        return -sample_ggx_visible_normal(&-*wo, alpha_x, alpha_y, sample);
    }

    let hemi_wo = Vec3::new(alpha_x * wo.x, alpha_y * wo.y, wo.z).normalize();

    let r = sample.x.sqrt();
    let phi = 2.0 * f64::consts::PI * sample.y;
    let t1 = r * phi.cos();
    let t2 = r * phi.sin();
    let s = (1.0 + hemi_wo.z) * 0.5;
    let t2 = (1.0 - s) * (1.0 - t1 * t1).sqrt() + s * t2;
    let disk_n = Vec3::new(t1, t2, (1.0 - t1 * t1 - t2 * t2).max(0.0).sqrt());

    let hemi_frame = Frame::new(&hemi_wo);
    let hemi_n = hemi_frame.to_world(&disk_n);

    Vec3::new(alpha_x * hemi_n.x, alpha_y * hemi_n.y, hemi_n.z.max(0.0)).normalize()
}
