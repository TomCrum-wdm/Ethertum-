struct Params {
    terrain_mode: u32,
    octaves: u32,
    len: u32,
    total_voxels: u32,

    seed_lo: u32,
    seed_hi: u32,
    superflat_ground_level: i32,
    superflat_dirt_depth: i32,

    noise_scale_2d: f32,
    noise_scale_3d: f32,
    flat_height_divisor: f32,
    flat_3d_noise_strength: f32,

    flat_water_level: f32,
    planet_radius: f32,
    planet_shell_thickness: f32,
    planet_3d_noise_strength: f32,

    planet_inner_water: u32,
    superflat_water_level: i32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,

    planet_center: vec4<f32>,
};

@group(0) @binding(0) var<storage, read> chunk_positions: array<vec4<i32>>;
@group(0) @binding(1) var<storage, read_write> vox_out: array<vec2<u32>>;
@group(0) @binding(2) var<uniform> params: Params;

fn hash31(p: vec3<f32>) -> f32 {
    let h = dot(p, vec3<f32>(127.1, 311.7, 74.7));
    return fract(sin(h) * 43758.5453);
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    return a + (b - a) * t;
}

fn value_noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);

    let v000 = hash31(i + vec3<f32>(0.0, 0.0, 0.0));
    let v100 = hash31(i + vec3<f32>(1.0, 0.0, 0.0));
    let v010 = hash31(i + vec3<f32>(0.0, 1.0, 0.0));
    let v110 = hash31(i + vec3<f32>(1.0, 1.0, 0.0));
    let v001 = hash31(i + vec3<f32>(0.0, 0.0, 1.0));
    let v101 = hash31(i + vec3<f32>(1.0, 0.0, 1.0));
    let v011 = hash31(i + vec3<f32>(0.0, 1.0, 1.0));
    let v111 = hash31(i + vec3<f32>(1.0, 1.0, 1.0));

    let x00 = lerp(v000, v100, f.x);
    let x10 = lerp(v010, v110, f.x);
    let x01 = lerp(v001, v101, f.x);
    let x11 = lerp(v011, v111, f.x);

    let y0 = lerp(x00, x10, f.y);
    let y1 = lerp(x01, x11, f.y);

    return lerp(y0, y1, f.z) * 2.0 - 1.0;
}

fn fbm3(p: vec3<f32>, octaves: u32) -> f32 {
    var acc = 0.0;
    var amp = 1.0;
    var freq = 1.0;

    for (var i: u32 = 0u; i < 12u; i = i + 1u) {
        if (i >= octaves) {
            break;
        }
        acc = acc + value_noise3(p * freq) * amp;
        amp = amp * 0.5;
        freq = freq * 2.0;
    }

    return acc;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.total_voxels) {
        return;
    }

    let len = params.len;
    let vox_per_chunk = len * len * len;
    let chunk_i = idx / vox_per_chunk;
    let local_i = idx % vox_per_chunk;

    let lx = i32(local_i / (len * len));
    let ly = i32((local_i / len) % len);
    let lz = i32(local_i % len);

    let chunkpos = chunk_positions[chunk_i].xyz;
    let p_i32 = chunkpos + vec3<i32>(lx, ly, lz);
    let p = vec3<f32>(f32(p_i32.x), f32(p_i32.y), f32(p_i32.z));

    let seed2_x = f32(i32(params.seed_lo & 1023u)) - 512.0;
    let seed2_z = f32(i32(params.seed_hi & 1023u)) - 512.0;
    let seed3_x = f32(i32((params.seed_lo >> 10u) & 1023u)) - 512.0;
    let seed3_y = f32(i32((params.seed_lo >> 20u) & 1023u)) - 512.0;
    let seed3_z = f32(i32((params.seed_hi >> 10u) & 1023u)) - 512.0;

    let noise2d = fbm3(
        vec3<f32>(
            (p.x + seed2_x) / params.noise_scale_2d,
            0.0,
            (p.z + seed2_z) / params.noise_scale_2d
        ),
        params.octaves
    );
    let noise3d = fbm3(
        vec3<f32>(
            (p.x + seed3_x) / params.noise_scale_3d,
            (p.y + seed3_y) / params.noise_scale_3d,
            (p.z + seed3_z) / params.noise_scale_3d
        ),
        params.octaves
    );

    var val = 0.0;
    var tex: u32 = 0u;

    if (params.terrain_mode == 0u) {
        let d = distance(p, params.planet_center.xyz);
        val = noise2d - ((d - params.planet_radius) / max(params.planet_shell_thickness, 1.0)) + noise3d * params.planet_3d_noise_strength;

        if (val > 0.0) {
            tex = 22u;
        } else if (params.planet_inner_water != 0u && d < params.planet_radius && val < 0.0) {
            val = -0.1;
            tex = 24u;
        }
    } else if (params.terrain_mode == 1u) {
        val = noise2d - (p.y / params.flat_height_divisor) + noise3d * params.flat_3d_noise_strength;

        if (val > 0.0) {
            tex = 22u;
        } else if (p.y < params.flat_water_level && val < 0.0) {
            val = -0.1;
            tex = 24u;
        }
    } else {
        let stone_top = params.superflat_ground_level - max(params.superflat_dirt_depth, 1);
        let py = p_i32.y;
        if (py < stone_top) {
            val = 1.0;
            tex = 22u;
        } else if (py < params.superflat_ground_level) {
            val = 1.0;
            tex = 1u;
        } else if (py == params.superflat_ground_level) {
            val = 1.0;
            tex = 12u;
        } else if (py <= params.superflat_water_level) {
            val = -0.1;
            tex = 24u;
        } else {
            val = -1.0;
            tex = 0u;
        }
    }

    vox_out[idx] = vec2<u32>(bitcast<u32>(val), tex);
}
