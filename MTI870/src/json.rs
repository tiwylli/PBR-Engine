use std::collections::HashMap;

use cgmath::{InnerSpace, Matrix, SquareMatrix, Zero};
use tinyjson::JsonValue;

pub fn json_to_f64(json: &HashMap<String, JsonValue>, name: &str, default: f64) -> f64 {
    if !json.contains_key(name) {
        return default;
    }

    let json_val = &json[name];
    match json_val {
        JsonValue::Number(v) => *v,
        _ => default,
    }
}

pub fn json_to_bool(json: &HashMap<String, JsonValue>, name: &str, default: bool) -> bool {
    if !json.contains_key(name) {
        return default;
    }

    let json_val = &json[name];
    match json_val {
        JsonValue::Boolean(v) => *v,
        _ => default,
    }
}

pub fn json_to_string(json: &HashMap<String, JsonValue>, name: &str, default: &str) -> String {
    if !json.contains_key(name) {
        return default.to_string();
    }

    let json_val = &json[name];
    match json_val {
        JsonValue::String(v) => v.clone(),
        _ => default.to_string(),
    }
}

pub fn json_to_vec3s(json: &Vec<JsonValue>) -> crate::Result<Vec<Vec3>> {
    let mut v = Vec::new();
    for json_val in json {
        match json_val {
            JsonValue::Array(json) => {
                if json.len() == 3 {
                    let values: Vec<f64> = json[0..3].iter().map(|v| *v.get().unwrap()).collect();
                    v.push(Vec3::new(values[0], values[1], values[2]));
                } else {
                    return Err(crate::Error::WrongDimensionJson("vec3", json.to_vec(), 3));
                }
            }
            _ => {
                return Err(crate::Error::InvalidType(
                    "Conversion to vec3 is impossible here".to_string(),
                ))
            }
        }
    }
    Ok(v)
}

use crate::{
    deg2rad,
    vec::{Mat4, Vec2, Vec2i, Vec3, Vec4},
};
struct JsonVec3(Vec3);
impl TryFrom<JsonValue> for JsonVec3 {
    type Error = crate::Error;

    fn try_from(value: JsonValue) -> Result<Self, Self::Error> {
        match value {
            JsonValue::Number(v) => Ok(JsonVec3(Vec3::new(v, v, v))),
            JsonValue::Array(v) => {
                if v.len() == 3 {
                    let v: Vec<f64> = v[0..3].iter().map(|v| *v.get().unwrap()).collect();
                    Ok(JsonVec3(Vec3::new(v[0], v[1], v[2])))
                } else {
                    Err(crate::Error::WrongDimensionJson("vec3", v, 3))
                }
            }
            _ => Err(crate::Error::UncoveredCaseJson("vec3", value)),
        }
    }
}

struct JsonVec2(Vec2);
impl TryFrom<JsonValue> for JsonVec2 {
    type Error = crate::Error;

    fn try_from(value: JsonValue) -> Result<Self, Self::Error> {
        match value {
            JsonValue::Number(v) => Ok(JsonVec2(Vec2::new(v, v))),
            JsonValue::Array(v) => {
                if v.len() == 2 {
                    let v: Vec<f64> = v[0..2].iter().map(|v| *v.get().unwrap()).collect();
                    Ok(JsonVec2(Vec2::new(v[0], v[1])))
                } else {
                    Err(crate::Error::WrongDimensionJson("vec2", v, 2))
                }
            }
            _ => Err(crate::Error::UncoveredCaseJson("vec2", value)),
        }
    }
}

struct JsonVec2i(Vec2i);
impl TryFrom<JsonValue> for JsonVec2i {
    type Error = crate::Error;

    fn try_from(value: JsonValue) -> Result<Self, Self::Error> {
        match value {
            JsonValue::Number(v) => {
                let v = v as i32;
                Ok(JsonVec2i(Vec2i::new(v, v)))
            }
            JsonValue::Array(v) => {
                if v.len() == 2 {
                    let v: Vec<i32> = v[0..2]
                        .iter()
                        .map(|v| *v.get::<f64>().unwrap() as i32)
                        .collect();
                    Ok(JsonVec2i(Vec2i::new(v[0], v[1])))
                } else {
                    Err(crate::Error::WrongDimensionJson("vec2i", v, 2))
                }
            }
            _ => Err(crate::Error::UncoveredCaseJson("vec2", value)),
        }
    }
}

pub fn json_to_vec3(json: &HashMap<String, JsonValue>, name: &str, default: Vec3) -> Vec3 {
    if !json.contains_key(name) {
        return default;
    }

    let json_val = json[name].clone();
    match TryInto::<JsonVec3>::try_into(json_val) {
        Err(_) => default,
        Ok(v) => v.0,
    }
}

pub fn json_to_vec2i(json: &HashMap<String, JsonValue>, name: &str, default: Vec2i) -> Vec2i {
    if !json.contains_key(name) {
        return default;
    }

    let json_val = json[name].clone();
    match TryInto::<JsonVec2i>::try_into(json_val) {
        Err(_) => default,
        Ok(v) => v.0,
    }
}

pub fn json_to_vec2(json: &HashMap<String, JsonValue>, name: &str, default: Vec2) -> Vec2 {
    if !json.contains_key(name) {
        return default;
    }

    let json_val = json[name].clone();
    match TryInto::<JsonVec2>::try_into(json_val) {
        Err(_) => default,
        Ok(v) => v.0,
    }
}

fn json_to_mat4_single(json: &HashMap<String, JsonValue>) -> crate::Result<Mat4> {
    if json.contains_key("from")
        || json.contains_key("to")
        || json.contains_key("up")
        || json.contains_key("at")
    {
        let from = json_to_vec3(json, "from", Vec3::new(0.0, 0.0, 1.0));
        let to = json_to_vec3(json, "to", Vec3::new(0.0, 0.0, 0.0));
        let to = to + json_to_vec3(json, "at", Vec3::new(0.0, 0.0, 0.0));
        let up = json_to_vec3(json, "up", Vec3::new(0.0, 1.0, 0.0));

        let dir = (from - to).normalize();
        let left = up.cross(dir).normalize();
        let up = dir.cross(left).normalize();

        Ok(Mat4::from_cols(
            Vec4::new(left.x, left.y, left.z, 0.0),
            Vec4::new(up.x, up.y, up.z, 0.0),
            Vec4::new(dir.x, dir.y, dir.z, 0.0),
            Vec4::new(from.x, from.y, from.z, 1.0),
        ))
    } else if json.contains_key("o")
        || json.contains_key("x")
        || json.contains_key("y")
        || json.contains_key("z")
    {
        let o = json_to_vec3(json, "o", Vec3::zero());
        let x = json_to_vec3(json, "x", Vec3::new(1.0, 0.0, 0.0));
        let y = json_to_vec3(json, "y", Vec3::new(0.0, 1.0, 0.0));
        let z = json_to_vec3(json, "z", Vec3::new(0.0, 0.0, 1.0));
        Ok(Mat4::from_cols(
            Vec4::new(x.x, x.y, x.z, 0.0),
            Vec4::new(y.x, y.y, y.z, 0.0),
            Vec4::new(z.x, z.y, z.z, 0.0),
            Vec4::new(o.x, o.y, o.z, 1.0),
        ))
    } else if json.contains_key("translate") {
        let t = json_to_vec3(json, "translate", Vec3::zero());
        Ok(Mat4::from_translation(t))
    } else if json.contains_key("scale") {
        let value = json_to_vec3(json, "scale", Vec3::zero());
        Ok(Mat4::from_diagonal(Vec4::new(
            value.x, value.y, value.z, 1.0,
        )))
    } else if json.contains_key("rotation") {
        // YXZ
        let r = json_to_vec3(json, "rotation", Vec3::zero());
        let r = r * crate::constants::M_PI / 180.0;
        let c = Vec3::new(r.x.cos(), r.y.cos(), r.z.cos());
        let s = Vec3::new(r.x.sin(), r.y.sin(), r.z.sin());

        Ok(Mat4::from_cols(
            Vec4::new(
                c[1] * c[2] - s[1] * s[0] * s[2],
                -c[1] * s[2] - s[1] * s[0] * c[2],
                -s[1] * c[0],
                0.0,
            ),
            Vec4::new(c[0] * s[2], c[0] * c[2], -s[0], 0.0),
            Vec4::new(
                s[1] * c[2] + c[1] * s[0] * s[2],
                -s[1] * s[2] + c[1] * s[0] * c[2],
                c[1] * c[0],
                0.0,
            ),
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        )
        .transpose())
    } else if json.contains_key("axis") || json.contains_key("angle") {
        let angle = deg2rad(json_to_f64(json, "angle", 0.0));
        let axis = json_to_vec3(json, "axis", Vec3::new(1.0, 0.0, 0.0));
        Ok(Mat4::from_axis_angle(axis.normalize(), cgmath::Rad(angle)))
    } else if json.contains_key("matrix") {
        let m: Vec<f64> = json["matrix"]
            .get::<Vec<JsonValue>>()
            .unwrap()
            .into_iter()
            .map(|v| *v.get().unwrap())
            .collect();
        Ok(Mat4::new(
            m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8], m[9], m[10], m[11], m[12], m[13],
            m[14], m[15],
        )
        .transpose())
    } else {
        Err(crate::Error::UncoveredCase("mat4", json.clone()))
    }
}

struct JsonMat4(Mat4);
impl TryFrom<JsonValue> for JsonMat4 {
    type Error = crate::Error;

    fn try_from(json: JsonValue) -> crate::Result<Self> {
        match json {
            JsonValue::Object(json) => {
                let m = json_to_mat4_single(&json)?;
                Ok(JsonMat4(m))
            }
            JsonValue::Array(vs) => {
                let mut m = Mat4::identity();
                for v in vs {
                    let mv: Result<HashMap<_, _>, _> = v.try_into();
                    if let Err(mv) = mv {
                        return Err(crate::Error::Other(Box::new(mv)));
                    }

                    let mv = json_to_mat4_single(&mv.unwrap())?;
                    m = mv * m;
                }
                Ok(JsonMat4(m))
            }
            _ => Err(crate::Error::UncoveredCaseJson("mat4", json)),
        }
    }
}

pub fn json_to_mat4(json: &HashMap<String, JsonValue>, name: &str) -> Option<Mat4> {
    if !json.contains_key(name) {
        return None;
    }

    let json_val = json[name].clone();
    let v: JsonMat4 = json_val.try_into().unwrap();
    Some(v.0)
}

// Merge two JSON
pub fn merge_json(
    json: &mut HashMap<String, JsonValue>,
    add: &HashMap<String, JsonValue>,
) -> crate::Result<()> {
    for (name, value) in add {
        // If the name is not found
        if !json.contains_key(name) {
            json.insert(name.clone(), value.clone());
        }

        // Two case now:
        // 1) If the value is a object, no problem, we can call this function recursively
        // 2) Otherwise, we replace the value
        let json_child = json.get_mut(name).unwrap();
        if value.is_object() {
            if !json_child.is_object() {
                return Err(crate::Error::FailedPatchJson(
                    json_child.clone(),
                    value.clone(),
                ));
            }
            merge_json(json_child.get_mut().unwrap(), value.get().unwrap())?;
        } else {
            *json_child = value.clone();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use cgmath::{assert_abs_diff_eq, SquareMatrix};
    use tinyjson::JsonValue;

    use crate::{
        json::JsonMat4,
        vec::{Mat4, Point3, Vec3, Vec4},
    };

    use super::JsonVec3;

    #[test]
    fn json_vec_3() {
        let s = r#"[1.0, 2.0, 3.0]"#;
        let parsed: JsonValue = s.parse().unwrap();
        let v: JsonVec3 = parsed.try_into().unwrap();
        assert_eq!(v.0, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn json_vec_3_float() {
        let s = r#"1.0"#;
        let parsed: JsonValue = s.parse().unwrap();
        let v: JsonVec3 = parsed.try_into().unwrap();
        assert_eq!(v.0, Vec3::new(1.0, 1.0, 1.0));
    }

    #[test]
    fn serde_look_at_transform() {
        let s = r#"
        {
            "from": [5.0, 15.0, -25.0], 
            "up": [0.0, 1.0, 0.0]
        }"#;
        let parsed: JsonValue = s.parse().unwrap();
        let v: JsonMat4 = parsed.try_into().unwrap();
        let t = Mat4::look_at_rh(
            Point3::new(5.0, 15.0, -25.0),
            Point3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        )
        .invert()
        .unwrap();
        assert_abs_diff_eq!(v.0, t);
    }

    #[test]
    fn serde_translate() {
        let s = r#"
        {
            "translate": [0.5, 0.5, -0.5]
        }"#;
        let parsed: JsonValue = s.parse().unwrap();
        let v: JsonMat4 = parsed.try_into().unwrap();
        let t = Mat4::from_translation(Vec3::new(0.5, 0.5, -0.5));
        assert_eq!(v.0, t);
    }

    #[test]
    fn serde_stacked_transforms() {
        let s = r#"[
            {
                "translate": [-0.5, 0, 0]
            }, {
                "scale": [0.75, 0.75, 0.75]  
            }, {
                "translate": [-1.0, -0.25, -1]
            }
        ]"#;
        let parsed: JsonValue = s.parse().unwrap();
        let v: JsonMat4 = parsed.try_into().unwrap();

        let t1 = Mat4::from_translation(Vec3::new(-0.5, 0.0, 0.0));
        let t2 = Mat4::from_diagonal(Vec4::new(0.75, 0.75, 0.75, 1.0));
        let t3 = Mat4::from_translation(Vec3::new(-1.0, -0.25, -1.0));

        assert_eq!(v.0, t1 * t2 * t3);
    }
}
