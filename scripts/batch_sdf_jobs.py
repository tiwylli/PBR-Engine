import argparse
import json
import os
import sys
import time
from pathlib import Path
from typing import Any, Dict, Iterable, List, Sequence

import numpy as np
import trimesh
from skimage import measure


ROOT = Path(__file__).resolve().parent.parent
sys.path.append(str(ROOT / "src/sdf_shapes"))
import sdf_extraction as sdf  # noqa: E402  # pylint: disable=wrong-import-position


DEFAULT_BOUNDS = ((-1.5, 1.5), (-1.5, 1.5), (-1.5, 1.5))

DEFAULT_JOBS: List[Dict[str, Any]] = [
    {
        "name": "mandelbulb_power8",
        "shape": {"type": "sdf_mandelbulb", "power": 8.0, "max_iterations": 16, "bailout": 8.0},
        "resolution": 320,
    },
    {
        "name": "mandelbulb_power6",
        "shape": {"type": "sdf_mandelbulb", "power": 6.0, "max_iterations": 18, "bailout": 10.0},
        "resolution": 384,
    },
    {
        "name": "julia_default",
        "shape": {"type": "sdf_julia", "constant": [0.355, 0.355, 0.355], "max_iterations": 28},
        "resolution": 320,
    },
    {
        "name": "julia_shifted",
        "shape": {"type": "sdf_julia", "constant": [-0.2, 0.6, 0.35], "max_iterations": 32},
        "resolution": 360,
    },
    {
        "name": "fbm_noise_dense",
        "shape": {
            "type": "sdf_fbm_noise",
            "half_extent": [1.0, 1.0, 1.0],
            "corner_radius": 0.15,
            "octaves": 8,
            "frequency": 2.2,
            "gain": 0.55,
            "blend": 0.2,
            "noise_variant": "simplex",
            "warp_matrix": [
                [0.0, 0.80, 0.60],
                [-0.80, 0.36, -0.48],
                [-0.60, -0.48, 0.64],
            ],
        },
        "resolution": 256,
    },
]


def linspace_axes(resolution: int, bounds: Sequence[Sequence[float]]):
    (xmin, xmax), (ymin, ymax), (zmin, zmax) = bounds
    xs = np.linspace(xmin, xmax, resolution, dtype=np.float32)
    ys = np.linspace(ymin, ymax, resolution, dtype=np.float32)
    zs = np.linspace(zmin, zmax, resolution, dtype=np.float32)
    return xs, ys, zs


def evaluate_field_chunked(
    resolution: int,
    bounds: Sequence[Sequence[float]],
    shape_spec: Dict[str, Any],
    chunk: int,
    scratch_dir: Path,
    dtype=np.float32,
):
    xs, ys, zs = linspace_axes(resolution, bounds)

    scratch_dir.mkdir(parents=True, exist_ok=True)
    field_path = scratch_dir / f"sdf_field_{shape_spec['type']}_{resolution}.dat"
    field = np.memmap(field_path, dtype=dtype, mode="w+", shape=(resolution, resolution, resolution))

    start = time.time()
    for z0 in range(0, resolution, chunk):
        z_slice = zs[z0 : z0 + chunk]
        grid = np.stack(np.meshgrid(xs, ys, z_slice, indexing="ij"), axis=-1).reshape(-1, 3)

        distances = sdf.evaluate_shape(grid, shape_spec).astype(dtype)
        field[:, :, z0 : z0 + len(z_slice)] = distances.reshape(resolution, resolution, len(z_slice))

        elapsed = time.time() - start
        completed = min(z0 + chunk, resolution)
        print(f"    slice {completed}/{resolution} written ({elapsed:.1f}s)", flush=True)

    field.flush()
    return field, xs, ys, zs, field_path


def marching_cubes_from_field(field: np.ndarray, xs: np.ndarray, ys: np.ndarray, zs: np.ndarray):
    dx = float(xs[1] - xs[0]) if len(xs) > 1 else 1.0
    dy = float(ys[1] - ys[0]) if len(ys) > 1 else 1.0
    dz = float(zs[1] - zs[0]) if len(zs) > 1 else 1.0
    verts, faces, normals, values = measure.marching_cubes(field, level=0.0, spacing=(dx, dy, dz))
    verts += np.array([xs[0], ys[0], zs[0]], dtype=np.float32)
    return verts, faces, normals, values


def run_job(job: Dict[str, Any], args):
    resolution = int(job.get("resolution", args.resolution))
    chunk = int(job.get("chunk", args.chunk))
    bounds = job.get("bounds", args.bounds)
    shape_spec = sdf.merge_defaults(job["shape"])

    name = job.get("name", f"{shape_spec['type']}_{resolution}")
    output = Path(args.output_dir) / f"{name}_{resolution}.obj"
    output.parent.mkdir(parents=True, exist_ok=True)

    print(f"[start] {name} res={resolution} chunk={chunk}")
    t0 = time.time()

    field, xs, ys, zs, field_path = evaluate_field_chunked(resolution, bounds, shape_spec, chunk, args.scratch_dir)
    verts, faces, normals, _ = marching_cubes_from_field(field, xs, ys, zs)
    mesh = trimesh.Trimesh(vertices=verts, faces=faces, vertex_normals=normals)
    mesh.export(output)

    # Clean up the memmap file to save disk space
    del field
    if field_path.exists():
        try:
            field_path.unlink()
        except OSError:
            pass

    elapsed = time.time() - t0
    print(f"[done ] {name} -> {output} ({elapsed / 60:.1f} min)")


def load_jobs(path: Path) -> List[Dict[str, Any]]:
    with open(path, "r", encoding="utf-8") as handle:
        data = json.load(handle)
    if not isinstance(data, list):
        raise ValueError("Job file must be a JSON array of job objects")
    return data


def parse_bounds(raw: Sequence[float]):
    if len(raw) != 6:
        raise argparse.ArgumentTypeError("Bounds must be 6 floats: xmin xmax ymin ymax zmin zmax")
    return ((raw[0], raw[1]), (raw[2], raw[3]), (raw[4], raw[5]))


def main(argv: Iterable[str] | None = None):
    parser = argparse.ArgumentParser(description="Batch SDF mesh extraction with chunked evaluation")
    parser.add_argument("--jobs-file", type=Path, default=None, help="JSON file describing jobs array")
    parser.add_argument("--use-default-jobs", action="store_true", help="Run built-in Mandelbulb/Julia/FBM jobs")
    parser.add_argument("--resolution", type=int, default=256, help="Default resolution if job omits it")
    parser.add_argument("--chunk", type=int, default=8, help="Z-slice chunk size to limit peak RAM")
    parser.add_argument(
        "--bounds",
        type=float,
        nargs=6,
        metavar=("XMIN", "XMAX", "YMIN", "YMAX", "ZMIN", "ZMAX"),
        default=sum(DEFAULT_BOUNDS, ()),
        help="Axis-aligned bounds sampled for all jobs unless overridden",
    )
    parser.add_argument("--output-dir", type=Path, default=ROOT / "outputs/batch_meshes", help="Where to write OBJs")
    parser.add_argument("--scratch-dir", type=Path, default=ROOT / "outputs/tmp_fields", help="Scratch for memmaps")

    args = parser.parse_args(argv)
    args.bounds = parse_bounds(args.bounds)

    jobs = []
    if args.use_default_jobs:
        jobs.extend(DEFAULT_JOBS)
    if args.jobs_file:
        jobs.extend(load_jobs(args.jobs_file))
    if not jobs:
        parser.error("No jobs specified. Use --use-default-jobs or provide --jobs-file.")

    for job in jobs:
        run_job(job, args)


if __name__ == "__main__":
    main()
