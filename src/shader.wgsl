
struct Camera {
    position: vec3<f32>,
    direction: vec3<f32>,
};

@group(0) @binding(0) var output: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(1) var<uniform> camera: Camera;

const ETA = 0.001;
const INFINITY = 3.402823466e+38;

const TYPE_SPHERE: u32 = 0u;
const TYPE_BOX: u32 = 1u;

struct Primitive {
    shape_type: u32,
    position: vec3<f32>,
    // Circles x is radius, box xyz is half extents etc.
    size: vec3<f32>,
    colour: vec3<f32>,
};

struct Ray {
    position: vec3<f32>,
    direction: vec3<f32>,
};

fn get_ray(pixel: vec2<f32>, resolution: vec2<f32>, c_pos: vec3<f32>, c_direction: vec3<f32>, fov_y_degrees: f32) -> Ray {
    // Convert to NDC (-1 to 1)
    let uv = (pixel + vec2<f32>(0.5)) / resolution; // 0.5 moves to pixel centre
    let aspect_ratio = resolution.x / resolution.y;

    // WGSL texture 0, 0 is top left
    let ndc = vec2<f32>(
        (uv.x * 2.0 - 1.0) * aspect_ratio,
        1.0 - uv.y * 2.0
    );

    // Camera
    let world_up = vec3<f32>(0.0, 1.0, 0.0);
    let forward = normalize(c_direction);
    let right = normalize(cross(forward, world_up));
    let up = cross(right, forward);

    // Scale with FOV
    let fov_y_radians = radians(fov_y_degrees);
    let half_height = tan(fov_y_radians * 0.5);

    // Ray direction
    let ray_direction = normalize(forward + right * (ndc.x * half_height) + up * (ndc.y * half_height));

    return Ray(c_pos, ray_direction);
}

fn sd_sphere(p: vec3<f32>, radius: f32) -> f32 {
    return length(p)-radius;
}

fn sd_box(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0)) + min(max(q.x, max(q.y, q.z)), 0.0));
}

fn evaluate_sd(ray_position: vec3<f32>, shape: Primitive) -> f32 {
    let p = ray_position - shape.position;

    switch shape.shape_type {
        case TYPE_SPHERE: {
            return sd_sphere(p, shape.size.x);
        }
        case TYPE_BOX: {
            return sd_box(p, shape.size);
        }
        default: {
            return 1e10;
        }
    }
}

@compute
@workgroup_size(8, 8)
fn main(
    @builtin(global_invocation_id) id: vec3<u32>
) {
    let dimensions = textureDimensions(output);
    if (id.x >= dimensions.x || id.y >= dimensions.y) {
        return;
    }

    // Constants
    let fov = 60.0;

    // Create ray 
    let resolution = vec2<f32>(dimensions);
    let pixel = vec2<f32>(id.xy);

    var ray = get_ray(pixel, resolution, camera.position, camera.direction, fov);

    // World 
    let circle = Primitive(TYPE_SPHERE, vec3<f32>(0.0), vec3<f32>(0.5), vec3<f32>(0.2, 0.3, 0.4));
    let box = Primitive(TYPE_BOX, vec3<f32>(0.0), vec3<f32>(0.2, 0.2, 3.4), vec3<f32>(0.4, 0.3, 0.2));

    let count = 2;
    let primitives = array<Primitive, 2>(circle, box);

    // Trace
    var colour = vec3<f32>(1.0);
    var distance = 0.0;
    while (distance < 10) {
        var min_dist = INFINITY;
        var hit_colour = colour;
        for (var j = 0; j < count; j += 1) {
            let dist = evaluate_sd(ray.position, primitives[j]);
            if (dist < min_dist) {
                min_dist = dist;
                hit_colour = primitives[j].colour;
            }
        }

        if (min_dist <= ETA) {
            colour = hit_colour;
            break;
        }

        let delta = ray.direction * min_dist;

        distance += length(delta);

        ray.position += delta;
    }

    // Fill pixel

    textureStore(output, vec2<i32>(id.xy), vec4<f32>(colour, 1.0));
}
