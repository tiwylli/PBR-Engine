import argparse
import copy
import json
import os
from typing import Any, Dict, List, Optional

import numpy as np
from scipy import spatial
from skimage import measure
import torch

import polyscope
import trimesh


def grid_to_mesh(grid_dict: Dict[str, Any], z: np.ndarray, scale: float = 1.0, translate=(0, 0, 0)):
    cell_width = grid_dict["xyz"][0][2] - grid_dict["xyz"][0][1]

    if z.ndim != 3:
        raise ValueError("Input z must be a 3D array.")

    verts, faces, normals, values = measure.marching_cubes(
        volume=z, level=0.0, spacing=(cell_width, cell_width, cell_width)
    )

    verts = verts + np.array([grid_dict["xyz"][0][0], grid_dict["xyz"][1][0], grid_dict["xyz"][2][0]])
    verts = verts * (1 / scale) - np.asarray(translate, dtype=np.float64)

    return verts, faces, normals, values


def smooth_max(a: np.ndarray, b: np.ndarray, k: float) -> np.ndarray:
    if k <= 0.0:
        return np.maximum(a, b)
    diff = np.abs(a - b)
    h = np.clip((k - diff) / k, 0.0, 1.0)
    return np.maximum(a, b) + 0.25 * h * h * k


def hash_vec3(values: np.ndarray) -> np.ndarray:
    arr = np.asarray(values, dtype=np.int64)
    reshaped = arr.reshape(-1, 3)
    n = reshaped[:, 0] * 15731 + reshaped[:, 1] * 789_221 + reshaped[:, 2] * 13_763_125_899
    n = (n << 13) ^ n
    nn = n * (n * n * 15731 + 789_221) + 13_763_125_899
    hashed = 1.0 - ((nn & 0x7FFF_FFFF) / 1_073_741_824.0)
    return hashed.reshape(arr.shape[:-1])


def sd_base_lattice(points: np.ndarray) -> np.ndarray:
    cell = np.floor(points)
    frac = points - cell
    result = np.full(points.shape[0], np.inf, dtype=np.float64)
    for dx in (0.0, 1.0):
        for dy in (0.0, 1.0):
            for dz in (0.0, 1.0):
                corner = np.array([dx, dy, dz], dtype=np.float64)
                hashes = hash_vec3(cell + corner)
                radius = np.square(np.abs(hashes)) * 0.7
                diff = frac - corner
                dist = np.linalg.norm(diff, axis=1) - radius
                result = np.minimum(result, dist)
    return result


def sd_base_simplex(points: np.ndarray) -> np.ndarray:
    k1 = 1.0 / 3.0
    k2 = 1.0 / 6.0
    sum_coords = np.sum(points, axis=1)
    i = np.floor(points + sum_coords[:, None] * k1)
    t = (np.sum(i, axis=1) * k2)[:, None]
    d0 = points - (i - t)

    ex = (d0[:, 1] < d0[:, 0]).astype(np.float64)
    ey = (d0[:, 2] < d0[:, 1]).astype(np.float64)
    ez = (d0[:, 0] < d0[:, 2]).astype(np.float64)

    i1 = np.stack([ex * (1.0 - ez), ey * (1.0 - ex), ez * (1.0 - ey)], axis=1)
    i2 = np.stack(
        [
            1.0 - ex * (1.0 - ez),
            1.0 - ey * (1.0 - ex),
            1.0 - ez * (1.0 - ey),
        ],
        axis=1,
    )

    d1 = d0 - i1 + k2
    d2 = d0 - i2 + 2.0 * k2
    d3 = d0 - 0.5

    r0 = hash_vec3(i)
    r1 = hash_vec3(i + i1)
    r2 = hash_vec3(i + i2)
    r3 = hash_vec3(i + 1.0)

    def sph(delta: np.ndarray, r: np.ndarray) -> np.ndarray:
        return np.linalg.norm(delta, axis=1) - 0.55 * np.square(r)

    return np.minimum.reduce([sph(d0, r0), sph(d1, r1), sph(d2, r2), sph(d3, r3)])


def sd_fbm(
    points: np.ndarray,
    base_distance: np.ndarray,
    octaves: int,
    frequency: float,
    gain: float,
    blend: float,
    warp_matrix: np.ndarray,
    variant: str,
) -> np.ndarray:
    result = base_distance.copy()
    amp = 1.0
    freq = 1.0
    warped = points.copy()
    use_simplex = variant.lower() == "simplex"

    for _ in range(octaves):
        base = sd_base_simplex(warped * freq) if use_simplex else sd_base_lattice(warped * freq)
        noise = amp * base
        result = smooth_max(result, -noise, blend * amp)
        warped = warped @ warp_matrix.T
        amp *= gain
        freq *= frequency

    return result


def sdf_sphere(points: np.ndarray, params: Dict[str, Any], *_args) -> np.ndarray:
    radius = float(params["radius"])
    return np.linalg.norm(points, axis=1) - radius


def sdf_plane(points: np.ndarray, params: Dict[str, Any], *_args) -> np.ndarray:
    normal = np.asarray(params["normal"], dtype=np.float64)
    norm = np.linalg.norm(normal)
    if norm <= 1e-12:
        normal = np.array([0.0, 1.0, 0.0], dtype=np.float64)
    else:
        normal = normal / norm
    offset = float(params["offset"])
    return points @ normal + offset


def sdf_round_box(points: np.ndarray, params: Dict[str, Any], *_args) -> np.ndarray:
    half_extent = np.asarray(params["half_extent"], dtype=np.float64)
    radius = float(params["radius"])
    q = np.abs(points) - half_extent + radius
    q_max = np.maximum(q, 0.0)
    inner = np.minimum(np.max(q, axis=1), 0.0)
    return np.linalg.norm(q_max, axis=1) + inner - radius


def sdf_sphere_sine(points: np.ndarray, params: Dict[str, Any], *_args) -> np.ndarray:
    radius = float(params["radius"])
    disp_freq = np.asarray(params["displacement_freq"], dtype=np.float64)
    disp_axis_amp = np.asarray(params["displacement_axis_amp"], dtype=np.float64)
    disp_amp = float(params["displacement_amp"])
    disp_x = np.sin(disp_freq[0] * points[:, 0]) * disp_axis_amp[0]
    disp_y = np.sin(disp_freq[1] * points[:, 1]) * disp_axis_amp[1]
    disp_z = np.sin(disp_freq[2] * points[:, 2]) * disp_axis_amp[2]
    displacement = disp_x * disp_y * disp_z
    base = np.linalg.norm(points, axis=1) - radius
    return base + disp_amp * displacement


def sdf_fbm_noise(points: np.ndarray, params: Dict[str, Any], *_args) -> np.ndarray:
    half_extent = np.asarray(params["half_extent"], dtype=np.float64)
    corner_radius = float(params["corner_radius"])
    offset = np.asarray(params["offset"], dtype=np.float64)
    octaves = int(params["octaves"])
    frequency = float(params["frequency"])
    gain = float(params["gain"])
    blend = float(params["blend"])
    warp_matrix = np.asarray(params["warp_matrix"], dtype=np.float64)
    noise_variant = params["noise_variant"].lower()

    q = np.abs(points) - half_extent + corner_radius
    q_max = np.maximum(q, 0.0)
    inside = np.minimum(np.max(q, axis=1), 0.0)
    base = np.linalg.norm(q_max, axis=1) + inside - corner_radius

    return sd_fbm(points + offset, base, octaves, frequency, gain, blend, warp_matrix, noise_variant)


def sdf_fbm_noise_sphere(points: np.ndarray, params: Dict[str, Any], *_args) -> np.ndarray:
    if "radius" in params:
        radius = float(params["radius"])
    else:
        half_extent = np.asarray(params.get("half_extent", [1.0, 1.0, 1.0]), dtype=np.float64)
        corner_radius = float(params.get("corner_radius", 0.1))
        radius = float(np.max(half_extent) + corner_radius)
    offset = np.asarray(params["offset"], dtype=np.float64)
    octaves = int(params["octaves"])
    frequency = float(params["frequency"])
    gain = float(params["gain"])
    blend = float(params["blend"])
    warp_matrix = np.asarray(params["warp_matrix"], dtype=np.float64)
    noise_variant = params["noise_variant"].lower()

    base = np.linalg.norm(points, axis=1) - radius

    return sd_fbm(points + offset, base, octaves, frequency, gain, blend, warp_matrix, noise_variant)


def sdf_menger_sponge(points: np.ndarray, params: Dict[str, Any], *_args) -> np.ndarray:
    half_size = max(float(params["half_size"]), 1.0e-8)
    iterations = max(int(params["iterations"]), 1)

    p = points / half_size
    q = np.abs(p) - 1.0
    outside = np.maximum(q, 0.0)
    inside = np.minimum(np.max(q, axis=1), 0.0)
    d = np.linalg.norm(outside, axis=1) + inside

    scale = 1.0
    for _ in range(iterations):
        p = (p * 3.0) % 2.0 - 1.0
        scale *= 3.0
        r = 1.0 - 3.0 * np.abs(p)
        da = np.maximum(np.abs(r[:, 0]), np.abs(r[:, 1]))
        db = np.maximum(np.abs(r[:, 1]), np.abs(r[:, 2]))
        dc = np.maximum(np.abs(r[:, 2]), np.abs(r[:, 0]))
        c = (np.minimum(np.minimum(da, db), dc) - 1.0) / scale
        d = np.maximum(d, c)

    return d * half_size


def sdf_capped_cylinder(points: np.ndarray, params: Dict[str, Any], *_args) -> np.ndarray:
    radius = float(params["radius"])
    half_height = float(params["half_height"])
    radial = np.linalg.norm(points[:, [0, 2]], axis=1)
    d = np.stack([radial - radius, np.abs(points[:, 1]) - half_height], axis=1)
    outside = np.linalg.norm(np.maximum(d, 0.0), axis=1)
    inside = np.minimum(np.maximum(d[:, 0], d[:, 1]), 0.0)
    return outside + inside


def sdf_mandelbulb(points: np.ndarray, params: Dict[str, Any], *_args) -> np.ndarray:
    power = float(params["power"])
    max_iterations = int(params["max_iterations"])
    bailout = float(params["bailout"])
    solid_radius = float(params["solid_radius"])
    z = points.copy()
    c = points
    dr = np.ones(points.shape[0], dtype=np.float64)
    r = np.linalg.norm(z, axis=1)
    eps = 1e-8

    for _ in range(max_iterations):
        active = (r > eps) & (r <= bailout)
        if not np.any(active):
            break

        z_active = z[active]
        r_active = r[active]
        theta = np.arccos(np.clip(z_active[:, 2] / r_active, -1.0, 1.0))
        phi = np.arctan2(z_active[:, 1], z_active[:, 0])
        zr = r_active**power
        dr_active = dr[active]
        dr_active = power * (r_active ** (power - 1.0)) * dr_active + 1.0

        theta *= power
        phi *= power
        sin_theta = np.sin(theta)
        cos_theta = np.cos(theta)
        cos_phi = np.cos(phi)
        sin_phi = np.sin(phi)

        new_z = np.empty_like(z_active)
        new_z[:, 0] = zr * sin_theta * cos_phi
        new_z[:, 1] = zr * sin_theta * sin_phi
        new_z[:, 2] = zr * cos_theta
        new_z += c[active]

        z[active] = new_z
        dr[active] = dr_active
        r[active] = np.linalg.norm(new_z, axis=1)

    dist = np.zeros(points.shape[0], dtype=np.float64)
    nonzero = r > eps
    dist[nonzero] = 0.5 * np.log(r[nonzero]) * r[nonzero] / dr[nonzero]

    if solid_radius > 0.0:
        inside = r < solid_radius
        dist[inside] = -(solid_radius - r[inside])

    return dist


def sdf_julia(points: np.ndarray, params: Dict[str, Any], *_args) -> np.ndarray:
    constant = np.asarray(params["constant"], dtype=np.float64)
    max_iterations = int(params["max_iterations"])
    bailout = float(params["bailout"])
    solid_radius = float(params["solid_radius"])
    power = 8.0
    z = points.copy()
    dr = np.ones(points.shape[0], dtype=np.float64)
    r = np.linalg.norm(z, axis=1)
    eps = 1e-8

    for _ in range(max_iterations):
        active = (r > eps) & (r <= bailout)
        if not np.any(active):
            break

        z_active = z[active]
        r_active = r[active]
        theta = np.arccos(np.clip(z_active[:, 2] / r_active, -1.0, 1.0))
        phi = np.arctan2(z_active[:, 1], z_active[:, 0])
        zr = r_active**power
        dr_active = dr[active]
        dr_active = power * (r_active ** (power - 1.0)) * dr_active + 1.0

        theta *= power
        phi *= power
        sin_theta = np.sin(theta)
        cos_theta = np.cos(theta)
        cos_phi = np.cos(phi)
        sin_phi = np.sin(phi)

        new_z = np.empty_like(z_active)
        new_z[:, 0] = zr * sin_theta * cos_phi
        new_z[:, 1] = zr * sin_theta * sin_phi
        new_z[:, 2] = zr * cos_theta
        new_z += constant

        z[active] = new_z
        dr[active] = dr_active
        r[active] = np.linalg.norm(new_z, axis=1)

    dist = np.zeros(points.shape[0], dtype=np.float64)
    nonzero = r > eps
    dist[nonzero] = 0.5 * np.log(r[nonzero]) * r[nonzero] / dr[nonzero]

    if solid_radius > 0.0:
        inside = r < solid_radius
        dist[inside] = -(solid_radius - r[inside])

    return dist


def sdf_union(points: np.ndarray, _params: Dict[str, Any], children: List[Dict[str, Any]], eval_child):
    if not children:
        raise ValueError("SDF union requires at least one child.")
    child_values = [eval_child(child) for child in children]
    return np.min(np.stack(child_values, axis=0), axis=0)


def sdf_intersection(points: np.ndarray, _params: Dict[str, Any], children: List[Dict[str, Any]], eval_child):
    if not children:
        raise ValueError("SDF intersection requires at least one child.")
    child_values = [eval_child(child) for child in children]
    return np.max(np.stack(child_values, axis=0), axis=0)


def sdf_difference(points: np.ndarray, _params: Dict[str, Any], children: List[Dict[str, Any]], eval_child):
    if len(children) != 2:
        raise ValueError("SDF difference expects exactly two children.")
    left = eval_child(children[0])
    right = eval_child(children[1])
    return np.maximum(left, -right)


SDF_EVALUATORS = {
    "sdf_sphere": sdf_sphere,
    "sdf_plane": sdf_plane,
    "sdf_round_box": sdf_round_box,
    "sdf_sphere_sine": sdf_sphere_sine,
    "sdf_fbm_noise": sdf_fbm_noise,
    "sdf_fbm_noise_sphere": sdf_fbm_noise_sphere,
    "sdf_menger_sponge": sdf_menger_sponge,
    "sdf_capped_cylinder": sdf_capped_cylinder,
    "sdf_mandelbulb": sdf_mandelbulb,
    "sdf_julia": sdf_julia,
    "sdf_union": sdf_union,
    "sdf_intersection": sdf_intersection,
    "sdf_difference": sdf_difference,
}


BASE_DEFAULTS = {"translate": [0.0, 0.0, 0.0], "scale": 1.0}

DEFAULT_SHAPE_PARAMS = {
    "sdf_sphere": {"radius": 1.0},
    "sdf_plane": {"normal": [0.0, 1.0, 0.0], "offset": 0.0},
    "sdf_round_box": {"half_extent": [1.0, 1.0, 1.0], "radius": 0.25},
    "sdf_sphere_sine": {
        "radius": 1.0,
        "displacement_freq": [1.0, 1.0, 1.0],
        "displacement_amp": 1.0,
        "displacement_axis_amp": [1.0, 1.0, 1.0],
    },
    "sdf_fbm_noise": {
        "half_extent": [1.0, 1.0, 1.0],
        "corner_radius": 0.1,
        "offset": [0.5, 0.5, 0.5],
        "octaves": 6,
        "frequency": 2.0,
        "gain": 0.55,
        "blend": 0.15,
        "warp_matrix": [
            [0.0, 0.80, 0.60],
            [-0.80, 0.36, -0.48],
            [-0.60, -0.48, 0.64],
        ],
        "noise_variant": "lattice",
    },
    "sdf_fbm_noise_sphere": {
        "radius": 1.1,
        "offset": [0.5, 0.5, 0.5],
        "octaves": 6,
        "frequency": 2.0,
        "gain": 0.55,
        "blend": 0.15,
        "warp_matrix": [
            [0.0, 0.80, 0.60],
            [-0.80, 0.36, -0.48],
            [-0.60, -0.48, 0.64],
        ],
        "noise_variant": "lattice",
    },
    "sdf_menger_sponge": {
        "half_size": 1.0,
        "iterations": 4,
    },
    "sdf_capped_cylinder": {"radius": 1.0, "half_height": 1.0},
    "sdf_mandelbulb": {"power": 8.0, "max_iterations": 12, "bailout": 8.0, "solid_radius": 0.0},
    "sdf_julia": {
        "constant": [0.355, 0.355, 0.355],
        "max_iterations": 25,
        "bailout": 6.0,
        "solid_radius": 0.0,
    },
    "sdf_union": {},
    "sdf_intersection": {},
    "sdf_difference": {},
}


def merge_defaults(spec: Dict[str, Any]) -> Dict[str, Any]:
    if "type" not in spec:
        raise ValueError("Shape specification is missing `type` field.")
    shape_type = spec["type"]
    if shape_type not in SDF_EVALUATORS:
        raise ValueError(f"Unknown shape type `{shape_type}`.")

    merged = {"type": shape_type}
    for key, value in BASE_DEFAULTS.items():
        merged[key] = copy.deepcopy(value)

    defaults = DEFAULT_SHAPE_PARAMS.get(shape_type, {})
    for key, value in defaults.items():
        merged[key] = copy.deepcopy(value)

    for key, value in spec.items():
        if key in {"type", "children"}:
            continue
        merged[key] = value

    if "children" in spec:
        merged["children"] = [merge_defaults(child) for child in spec["children"]]

    return merged


def evaluate_shape(points: np.ndarray, spec: Dict[str, Any]) -> np.ndarray:
    resolved = merge_defaults(spec)
    pts = np.asarray(points, dtype=np.float64)
    if pts.ndim != 2 or pts.shape[1] != 3:
        raise ValueError("Points must be of shape (N, 3).")
    return _evaluate_shape_recursive(pts, resolved)


def _evaluate_shape_recursive(points: np.ndarray, spec: Dict[str, Any]) -> np.ndarray:
    data = dict(spec)
    shape_type = data.pop("type")
    children = data.pop("children", [])
    translate = np.asarray(data.pop("translate"), dtype=np.float64)
    scale = float(data.pop("scale"))
    if scale <= 0.0:
        raise ValueError("Scale must be positive.")

    local_points = (points - translate) / scale
    evaluator = SDF_EVALUATORS[shape_type]

    def eval_child(child_spec: Dict[str, Any]) -> np.ndarray:
        return _evaluate_shape_recursive(local_points, child_spec)

    local_values = evaluator(local_points, data, children, eval_child)
    return local_values * scale


def load_shape_spec(shape_name: str, config_arg: Optional[str]) -> Dict[str, Any]:
    if config_arg is None:
        return {"type": shape_name}
    if os.path.isfile(config_arg):
        with open(config_arg, "r", encoding="utf-8") as handle:
            data = json.load(handle)
    else:
        data = json.loads(config_arg)
    if not isinstance(data, dict):
        raise ValueError("Shape configuration must be a JSON object.")
    data.setdefault("type", shape_name)
    return data


def marching_tetrahedra(points: np.ndarray, shape_spec: Dict[str, Any]):
    try:
        import torch
        import kaolin
    except ImportError as exc:  # pragma: no cover - optional dependency
        raise RuntimeError("Marching tetrahedra requires torch and kaolin to be installed.") from exc

    noise = np.random.normal(scale=1e-4, size=points.shape)
    noisy_points = points + noise
    delaunay = spatial.Delaunay(noisy_points)

    sdf_values = evaluate_shape(noisy_points, shape_spec)

    sdf_tensor = torch.tensor(sdf_values, dtype=torch.float32)
    points_tensor = torch.tensor(noisy_points, dtype=torch.float32)
    simplices_tensor = torch.tensor(delaunay.simplices, dtype=torch.long)

    vertices_list, faces_list = kaolin.ops.conversions.marching_tetrahedra(
        points_tensor.unsqueeze(0), simplices_tensor, sdf_tensor.unsqueeze(0), return_tet_idx=False
    )
    verts = vertices_list[0].detach().cpu().numpy()
    faces = faces_list[0].detach().cpu().numpy()
    return verts, faces


def reshape_grid_values(grid: Dict[str, Any], sdf_values: np.ndarray) -> np.ndarray:
    xs = grid["xyz"][0]
    ys = grid["xyz"][1]
    zs = grid["xyz"][2]
    return sdf_values.reshape(len(ys), len(xs), len(zs)).transpose(1, 0, 2).astype(np.float32)


def get_3d_grid(
    resolution=100, bbox=1.2 * np.array([[-1, 1], [-1, 1], [-1, 1]]), device=None, eps=0.1, dtype=np.float16
):
    # generate points on a uniform grid within  a given range
    # reimplemented from SAL : https://github.com/matanatz/SAL/blob/master/code/utils/plots.py
    # and IGR : https://github.com/amosgropp/IGR/blob/master/code/utils/plots.py

    shortest_axis = np.argmin(bbox[:, 1] - bbox[:, 0])
    if shortest_axis == 0:
        x = np.linspace(bbox[0, 0] - eps, bbox[0, 1] + eps, resolution)
        length = np.max(x) - np.min(x)
        y = np.arange(bbox[1, 0] - eps, bbox[1, 1] + length / (x.shape[0] - 1) + eps, length / (x.shape[0] - 1))
        z = np.arange(bbox[2, 0] - eps, bbox[2, 1] + length / (x.shape[0] - 1) + eps, length / (x.shape[0] - 1))
    elif shortest_axis == 1:
        y = np.linspace(bbox[1, 0] - eps, bbox[1, 1] + eps, resolution)
        length = np.max(y) - np.min(y)
        x = np.arange(bbox[0, 0] - eps, bbox[0, 1] + length / (y.shape[0] - 1) + eps, length / (y.shape[0] - 1))
        z = np.arange(bbox[2, 0] - eps, bbox[2, 1] + length / (y.shape[0] - 1) + eps, length / (y.shape[0] - 1))
    elif shortest_axis == 2:
        z = np.linspace(bbox[2, 0] - eps, bbox[2, 1] + eps, resolution)
        length = np.max(z) - np.min(z)
        x = np.arange(bbox[0, 0] - eps, bbox[0, 1] + length / (z.shape[0] - 1) + eps, length / (z.shape[0] - 1))
        y = np.arange(bbox[1, 0] - eps, bbox[1, 1] + length / (z.shape[0] - 1) + eps, length / (z.shape[0] - 1))

    xx, yy, zz = np.meshgrid(x.astype(dtype), y.astype(dtype), z.astype(dtype))  #
    # grid_points = get_cuda_ifavailable(torch.tensor(np.vstack([xx.ravel(), yy.ravel(), zz.ravel()]).T, dtype=torch.float16),
    #                                          device=device)
    grid_points = torch.tensor(np.vstack([xx.ravel(), yy.ravel(), zz.ravel()]).T, dtype=torch.float16)
    return {
        "grid_points": grid_points,
        "shortest_axis_length": length,
        "xyz": [x, y, z],
        "shortest_axis_index": shortest_axis,
    }


def main():
    parser = argparse.ArgumentParser(description="Extract meshes from analytic SDFs.")
    parser.add_argument("--scale", type=float, default=1.0, help="Scaling factor applied to the mesh output.")
    parser.add_argument(
        "--translate",
        type=float,
        nargs=3,
        default=(0.0, 0.0, 0.0),
        help="World-space translation applied after scaling the mesh.",
    )
    parser.add_argument("--output", type=str, default="", help="Optional output mesh file path.")
    parser.add_argument("--viz", action="store_true", help="Visualize the mesh with polyscope.")
    parser.add_argument("--resolution", type=int, default=64, help="Resolution of the sampling grid.")
    parser.add_argument(
        "--method",
        type=str,
        choices=["mc", "mt"],
        default="mc",
        help="Mesh extraction algorithm: marching cubes (mc) or marching tetrahedra (mt).",
    )
    parser.add_argument(
        "--shape",
        type=str,
        default="sdf_sphere",
        choices=sorted(SDF_EVALUATORS.keys()),
        help="Analytic SDF to evaluate when no config is provided.",
    )
    parser.add_argument(
        "--shape-config",
        type=str,
        default=None,
        help="Path to a JSON file or inline JSON object describing the shape tree.",
    )

    args = parser.parse_args()
    shape_spec = load_shape_spec(args.shape, args.shape_config)
    grid = get_3d_grid(resolution=args.resolution)
    points = grid["grid_points"].reshape(-1, 3)

    if args.method == "mc":
        print("Evaluate SDF...")
        sdf_values = evaluate_shape(points, shape_spec)
        print("Reshape SDF values into grid...")
        field = reshape_grid_values(grid, sdf_values)
        print("Using marching cubes...")
        verts, faces, normals, values = grid_to_mesh(grid, field, scale=args.scale, translate=args.translate)
    else:
        print("Using marching tetrahedra...")
        verts, faces = marching_tetrahedra(points, shape_spec)
        normals = None
        values = None

    if args.output:
        print(f"Exporting mesh to {args.output}...")
        mesh = trimesh.Trimesh(vertices=verts, faces=faces, vertex_normals=normals)
        mesh.export(args.output)
        print("Export complete.")

    if args.viz:
        print("Visualizing mesh with polyscope...")
        polyscope.init()
        polyscope.register_surface_mesh("mesh", verts, faces)
        polyscope.show()


if __name__ == "__main__":
    main()
