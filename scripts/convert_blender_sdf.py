import argparse
import json
from pathlib import Path
from typing import Dict, List, Tuple

import numpy as np

DEFAULT_SDF_SETTINGS = {
    "hit_epsilon": 1.0e-3,
    "normal_epsilon": 5.0e-3,
    "step_clamp": 0.7,
    "max_steps": 512,
}

DEFAULT_MEDIUM_SETTINGS = {
    "type": "homogeneous",
    "sigma_a": [0.0, 0.0, 0.0],
    "sigma_s": [0.4, 0.4, 0.4],
    "density": 0.04,
    "phase": {"type": "henyey-greenstein", "g": 0.55},
}

DEFAULT_DENOISE_SETTINGS = {
    "denoise": "none",
    "denoising_artefacts": True,
}

PINK_CRYSTAL_MATERIAL = {
    "type": "principled_bsdf",
    "name": "pink_crystal",
    "base_color": [0.913, 0.562, 1.0],
    "specular_transmission": 0.95,
    "metallic": 0.3,
    "subsurface": 0.0,
    "specular": 0.5,
    "roughness": 0.1,
    "specular_tint": 0.9,
    "anisotropic": 0.0,
    "sheen": 0.0,
    "sheen_tint": 0.0,
    "clearcoat": 0.0,
    "clearcoat_gloss": 0.9,
    "eta": 1.9,
}

EMERALD_MATERIAL = {
    "type": "principled_bsdf",
    "name": "emerald",
    "base_color": [0.039, 0.682, 0.345],
    "specular_transmission": 0.3,
    "metallic": 0.0,
    "subsurface": 1.0,
    "specular": 0.5,
    "roughness": 0.3,
    "specular_tint": 0.9,
    "anisotropic": 0.0,
    "sheen": 0.0,
    "sheen_tint": 0.0,
    "clearcoat": 0.5,
    "clearcoat_gloss": 0.9,
    "eta": 1.5,
}

BLUE_CRYSTAL_MATERIAL = {
    "type": "principled_bsdf",
    "name": "blue_crystal",
    "base_color": [0.141, 0.223, 0.478],
    "specular_transmission": 0.0,
    "metallic": 0.8,
    "subsurface": 0.4,
    "specular": 0.5,
    "roughness": 0.3,
    "specular_tint": 0.2,
    "anisotropic": 0.0,
    "sheen": 0.0,
    "sheen_tint": 0.0,
    "clearcoat": 0.5,
    "clearcoat_gloss": 0.9,
    "eta": 1.5,
}

SPACESHIP_MATERIAL = {
    "type": "principled_bsdf",
    "base_color": {"type": "texture", "filename": "textures/GR2_BaseColor.jpg", "gamma": True, "scale": 1.0},
    "roughness": {"type": "texture", "filename": "textures/GR2_roughness.jpg", "gamma": True, "scale": 1.0},
    "metallic": {"type": "texture", "filename": "textures/GR2_metallic.jpg", "gamma": True, "scale": 1.0},
    "anisotropic": 1.0,
    "name": "000-0-0.001",
}

SPACESHIP_WINDOW_MATERIAL = {
    "type": "principled_bsdf",
    "base_color": 1.0,
    "roughness": 0.1,
    "specular_transmission": 1.0,
    "name": "001-0-1.001",
}

SDF_REPLACEMENT_MATERIALS = [PINK_CRYSTAL_MATERIAL, EMERALD_MATERIAL, BLUE_CRYSTAL_MATERIAL]


def load_jobs(paths: List[Path]) -> Dict[str, Dict]:
    jobs: Dict[str, Dict] = {}
    for path in paths:
        if not path.exists():
            continue
        data = json.loads(path.read_text())
        if not isinstance(data, list):
            raise ValueError(f"Job file {path} must contain a JSON array")
        for job in data:
            name = job.get("name")
            if not name:
                continue
            jobs[name] = job
    return jobs


def dedupe_diffuse_materials(materials: List[Dict]) -> List[Dict]:
    """Drop trailing diffuse duplicates that reuse an existing material name."""
    seen: set[str] = set()
    filtered: List[Dict] = []
    for mat in materials:
        name = mat.get("name") if isinstance(mat, dict) else None
        mtype = mat.get("type") if isinstance(mat, dict) else None
        if name and mtype == "diffuse" and name in seen:
            continue
        if name:
            seen.add(name)
        filtered.append(mat)
    return filtered


def upsert_material(materials: List[Dict], new_mat: Dict) -> None:
    """Replace a material by name or append it when missing."""
    name = new_mat.get("name")
    if not name:
        materials.append(new_mat)
        return

    replaced = False
    updated: List[Dict] = []
    for mat in materials:
        if isinstance(mat, dict) and mat.get("name") == name:
            if not replaced:
                updated.append(new_mat)
                replaced = True
            continue
        updated.append(mat)

    if not replaced:
        updated.append(new_mat)
    materials[:] = updated


def apply_material_replacements(scene: Dict) -> None:
    """Override materials for the first three SDF objects and spaceship assets."""
    materials = scene.get("materials")
    if not isinstance(materials, list):
        materials = []
        scene["materials"] = materials

    sdf_objects = scene.get("sdf_objects") or []
    for sdf_obj, mat in zip(sdf_objects, SDF_REPLACEMENT_MATERIALS):
        if not isinstance(sdf_obj, dict):
            continue
        mat_name = mat.get("name")
        if not mat_name:
            continue
        upsert_material(materials, mat)
        sdf_obj["material"] = mat_name

    upsert_material(materials, SPACESHIP_MATERIAL)
    upsert_material(materials, SPACESHIP_WINDOW_MATERIAL)


def expand_inputs(inputs: List[Path]) -> List[Path]:
    """Expand files/directories into a flat list of JSON files."""
    out: List[Path] = []
    for p in inputs:
        if p.is_dir():
            out.extend(sorted(p.glob("*.json")))
        else:
            out.append(p)
    return out


def normalize_material(mat: Dict) -> Dict:
    """Flatten normal_map wrappers so normal_map sits on the base material."""
    if not isinstance(mat, dict):
        return mat

    mat = mat.copy()
    mtype = mat.get("type")

    if mtype == "blend":
        if "matA" in mat:
            mat["matA"] = normalize_material(mat["matA"])
        if "matB" in mat:
            mat["matB"] = normalize_material(mat["matB"])
        return mat

    if mtype == "normal_map":
        base = mat.get("material")
        if not isinstance(base, dict):
            return mat
        base = normalize_material(base)
        if "normal_map" in mat:
            base["normal_map"] = mat["normal_map"]
        if "name" in mat and "name" not in base:
            base["name"] = mat["name"]
        return base

    return mat


def is_spot_mesh(shape: Dict) -> bool:
    if shape.get("type") != "mesh":
        return False
    fname = shape.get("filename", "")
    stem = Path(fname).stem
    return strip_mat_suffix(stem).startswith("spot_")


def flatten_mesh_path(fname: str) -> str:
    """Drop per-frame mesh subdir (meshes/<frame>/file.obj -> meshes/file.obj)."""
    path = Path(fname)
    parts = path.parts
    if len(parts) >= 3 and parts[0] == "meshes" and parts[1].isdigit():
        return str(Path(parts[0]) / path.name)
    return fname


def extract_spot_transform(transform: Dict | None) -> tuple[np.ndarray, np.ndarray]:
    # Return (position, direction) using the transform's translation and the quad's normal (+Z).
    if not transform or "matrix" not in transform:
        return np.zeros(3), np.array([0.0, 0.0, 1.0], dtype=float)

    mat = transform.get("matrix", [])
    if not isinstance(mat, list) or len(mat) != 16:
        return np.zeros(3), np.array([0.0, 0.0, 1.0], dtype=float)

    pos = translate_from_matrix(mat)
    mat4 = np.array(mat, dtype=float).reshape(4, 4)
    # The spot mesh is a quad lying in the XY plane with its normal along +Z.
    dir_z = mat4[:3, :3] @ np.array([0.0, 0.0, 1.0], dtype=float)
    norm = np.linalg.norm(dir_z)
    if norm < 1.0e-8:
        dir_z = np.array([0.0, 0.0, 1.0], dtype=float)
    else:
        dir_z = dir_z / norm
    return pos, dir_z


def strip_mat_suffix(stem: str) -> str:
    # Drop trailing _matX if present
    parts = stem.split("_mat")
    return parts[0] if len(parts) > 1 else stem


def match_job(stem: str, jobs: Dict[str, Dict]) -> Tuple[str, Dict] | Tuple[None, None]:
    clean = strip_mat_suffix(stem)
    for name, job in jobs.items():
        if clean.startswith(name):
            return name, job
    return None, None


def base_bounds(shape_spec: Dict) -> Tuple[np.ndarray, np.ndarray]:
    t = shape_spec.get("type", "")
    if t in {"sdf_mandelbulb", "sdf_julia"}:
        ext = 4.0
        return -ext * np.ones(3), ext * np.ones(3)
    if t == "sdf_fbm_noise":
        half = np.array(shape_spec.get("half_extent", [1.0, 1.0, 1.0]), dtype=float)
        corner = float(shape_spec.get("corner_radius", 0.1))
        margin = 1.0
        ext = half + corner + margin
        return -ext, ext
    if t == "sdf_fbm_noise_sphere":
        if "radius" in shape_spec:
            radius = float(shape_spec.get("radius", 1.0))
        else:
            half = np.array(shape_spec.get("half_extent", [1.0, 1.0, 1.0]), dtype=float)
            corner = float(shape_spec.get("corner_radius", 0.1))
            radius = float(np.max(half) + corner)
        margin = 1.0
        ext = radius + margin
        return -ext * np.ones(3), ext * np.ones(3)
    if t == "sdf_menger_sponge":
        half = float(shape_spec.get("half_size", 1.0))
        return -half * np.ones(3), half * np.ones(3)
    return -10.0 * np.ones(3), 10.0 * np.ones(3)


def translate_from_matrix(mat: List[float]) -> np.ndarray:
    # Blender export gives row-major 4x4; translation sits in indices 3,7,11
    if len(mat) != 16:
        return np.zeros(3)
    return np.array([mat[3], mat[7], mat[11]], dtype=float)


def transform_bounds(bmin: np.ndarray, bmax: np.ndarray, mat: List[float]) -> tuple[np.ndarray, np.ndarray]:
    # Apply the full 4x4 transform to the 8 AABB corners to capture rotation/scale/shear.
    corners = np.array(
        [
            [bmin[0], bmin[1], bmin[2], 1.0],
            [bmin[0], bmin[1], bmax[2], 1.0],
            [bmin[0], bmax[1], bmin[2], 1.0],
            [bmin[0], bmax[1], bmax[2], 1.0],
            [bmax[0], bmin[1], bmin[2], 1.0],
            [bmax[0], bmin[1], bmax[2], 1.0],
            [bmax[0], bmax[1], bmin[2], 1.0],
            [bmax[0], bmax[1], bmax[2], 1.0],
        ],
        dtype=float,
    )
    mat4 = np.array(mat, dtype=float).reshape(4, 4)
    transformed = (mat4 @ corners.T).T[:, :3]
    return transformed.min(axis=0), transformed.max(axis=0)


def world_bounds(shape_spec: Dict, transform: Dict | None) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
    local_min, local_max = base_bounds(shape_spec)
    if transform and isinstance(transform, dict) and "matrix" in transform:
        mat = transform.get("matrix", [])
        if isinstance(mat, list) and len(mat) == 16:
            world_min, world_max = transform_bounds(local_min, local_max, mat)
            return local_min, local_max, world_min, world_max
        # Fall back to pure translation if the matrix is malformed
        t = translate_from_matrix(transform["matrix"])
        local_min = local_min + t
        local_max = local_max + t
    return local_min, local_max, local_min, local_max


def scaled_settings(local_min: np.ndarray, local_max: np.ndarray, world_min: np.ndarray, world_max: np.ndarray) -> Dict:
    """Derive per-object SDF settings scaled to the object's world size."""
    settings = DEFAULT_SDF_SETTINGS.copy()
    local_span = float(np.max(np.abs(local_max - local_min)))
    world_span = float(np.max(np.abs(world_max - world_min)))
    if local_span > 1.0e-8:
        scale = world_span / local_span
        scale = float(np.clip(scale, 1.0e-3, 1.0e6))
        settings["hit_epsilon"] = max(settings["hit_epsilon"] * scale, 1.0e-8)
        settings["normal_epsilon"] = max(settings["normal_epsilon"] * scale, 1.0e-8)
    return settings


def convert_scene(
    scene_path: Path, jobs: Dict[str, Dict], output: Path, set_integrator: bool, replace_materials: bool = False
) -> None:
    scene = json.loads(scene_path.read_text())

    if isinstance(scene.get("materials"), list):
        normalized = [normalize_material(mat) for mat in scene["materials"]]
        scene["materials"] = dedupe_diffuse_materials(normalized)

    new_shapes = []
    new_sdf_objects = scene.get("sdf_objects", [])
    spot_materials: set[str] = set()

    for shape in scene.get("shapes", []):
        if shape.get("type") == "mesh":
            fname = shape.get("filename", "")
            flat = flatten_mesh_path(fname)
            if flat != fname:
                shape = shape.copy()
                shape["filename"] = flat

        if is_spot_mesh(shape):
            pos, direction = extract_spot_transform(shape.get("transform"))
            mat_name = shape.get("material")
            if isinstance(mat_name, str):
                spot_materials.add(mat_name)
            new_shapes.append(
                {
                    "type": "spotlight",
                    "position": pos.tolist(),
                    "direction": direction.tolist(),
                    "material": mat_name,
                }
            )
            continue

        if shape.get("type") != "mesh":
            new_shapes.append(shape)
            continue

        fname = shape.get("filename", "")
        stem = Path(fname).stem
        job_name, job = match_job(stem, jobs)
        if job is None:
            new_shapes.append(shape)
            continue

        shape_spec = job["shape"]
        sdf_obj: Dict = {"name": job_name, "type": shape_spec["type"]}
        sdf_obj.update({k: v for k, v in shape_spec.items() if k != "type"})
        if "material" in shape:
            sdf_obj["material"] = shape["material"]
        if "transform" in shape:
            sdf_obj["transform"] = shape["transform"]

        local_min, local_max, world_min, world_max = world_bounds(shape_spec, shape.get("transform"))
        sdf_obj["bounds"] = {"min": world_min.tolist(), "max": world_max.tolist()}
        sdf_obj["settings"] = scaled_settings(local_min, local_max, world_min, world_max)

        new_sdf_objects.append(sdf_obj)

    scene["shapes"] = new_shapes
    if new_sdf_objects:
        scene["sdf_objects"] = new_sdf_objects

    if isinstance(scene.get("materials"), list) and spot_materials:
        for mat in scene["materials"]:
            if not isinstance(mat, dict):
                continue
            if mat.get("name") in spot_materials and mat.get("type") == "diffuse_light":
                mat["type"] = "diffuse_spotlight"
                mat.setdefault("focus", 10.0)

    if replace_materials:
        apply_material_replacements(scene)

    if set_integrator:
        scene["integrator"] = {
            "type": "hybrid_vol_path_mis",
            "max_depth": 16,
            "sdf": DEFAULT_SDF_SETTINGS.copy(),
        }
    scene["medium"] = DEFAULT_MEDIUM_SETTINGS.copy()
    scene["denoise"] = DEFAULT_DENOISE_SETTINGS["denoise"]
    scene["denoising_artefacts"] = DEFAULT_DENOISE_SETTINGS["denoising_artefacts"]
    scene["camera"]["vfov"] = 75.0

    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(scene, indent=2))


def main():
    parser = argparse.ArgumentParser(description="Replace Blender mesh SDF proxies with analytic SDF objects.")
    parser.add_argument("--input", type=Path, required=True, nargs="+", help="Input scene JSON files (blender export)")
    parser.add_argument(
        "--jobs",
        type=Path,
        nargs="*",
        default=[Path("scripts/jobs_extensive.json"), Path("scripts/jobs_preview.json")],
        help="Job files describing SDF param presets",
    )
    parser.add_argument(
        "--output-dir", type=Path, default=Path("scenes/generated"), help="Where to write converted scenes"
    )
    parser.add_argument(
        "--keep-integrator", action="store_true", help="Do not overwrite integrator (defaults to hybrid_path_mis)"
    )
    parser.add_argument(
        "--replace-materials",
        action="store_true",
        help="Inject preset materials for the first three SDF objects and spaceship assets",
    )

    args = parser.parse_args()

    jobs = load_jobs(args.jobs)
    if not jobs:
        raise SystemExit("No jobs loaded; provide at least one jobs JSON file")

    input_paths = expand_inputs(args.input)
    if not input_paths:
        raise SystemExit("No input scenes found (files or *.json in provided directories)")

    for input_path in input_paths:
        output_path = args.output_dir / ("c" + input_path.name)
        convert_scene(
            input_path,
            jobs,
            output_path,
            set_integrator=not args.keep_integrator,
            replace_materials=args.replace_materials,
        )
        print(f"Converted {input_path} -> {output_path}")


if __name__ == "__main__":
    main()
