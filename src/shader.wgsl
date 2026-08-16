
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
const TYPE_MANDELBULB: u32 = 2u;

struct Primitive {
    shape_type: u32,
    position: vec3<f32>,
    // Circles x is radius, box xyz is half extents etc.
    size: vec4<f32>,
    colour: vec3<f32>,
};

struct World {
    count: i32,
    primitives: array<Primitive, 3>,
};

struct DistanceColour {
    distance: f32,
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

fn sd_round_box(p: vec3<f32>, b: vec4<f32>) -> f32 {
    let q = abs(p) - b.xyz + b.w;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0) - b.w;
}

fn sd_mandelbulb(p: vec3<f32>) -> f32 {
    var w = p;
    var dr = 1.0;
    var r = 0.0;

    for (var i = 0; i < 8; i++) {
        r = length(w);
        if (r > 4.0) {
            break;
        }

        // derivative
        dr = pow(r, 7.0) * 8.0 * dr + 1.0;

        // spherical coordinates
        let theta = acos(w.y / r);
        let phi = atan2(w.x, w.z);

        // power
        let zr = pow(r, 8.0);
        let theta8 = theta * 8.0;
        let phi8 = phi * 8.0;

        // back to Cartesian
        w = zr * vec3(
            sin(theta8) * sin(phi8),
            cos(theta8),
            sin(theta8) * cos(phi8)
        );

        // add c
        w += p;
    }

    return 0.5 * log(r) * r / dr;
}

fn evaluate_sd(ray_position: vec3<f32>, shape: Primitive) -> f32 {
    let p = ray_position - shape.position;

    switch shape.shape_type {
        case TYPE_SPHERE: {
            return sd_sphere(p, shape.size.x);
        }
        case TYPE_BOX: {
            return sd_round_box(p, shape.size);
        }
        case TYPE_MANDELBULB: {
            return sd_mandelbulb(p);
        }
        default: {
            return 1e10;
        }
    }
}

fn get_closest(position: vec3<f32>, world: World) -> DistanceColour {
    var min_dist = INFINITY;
    var colour = vec3<f32>(0.0);
    for (var i = 0; i < world.count; i += 1) {
        let d = evaluate_sd(position, world.primitives[i]);

        if (d < min_dist) {
            min_dist = d;
            colour = world.primitives[i].colour;
        }
    }

    return DistanceColour(min_dist, colour);
}

fn get_normal(position: vec3<f32>, world: World) -> vec3<f32> {
    let e = vec2<f32>(0.001, 0.0);

    let n = vec3<f32>(
        get_closest(position+e.xyy, world).distance - get_closest(position-e.xyy, world).distance,
        get_closest(position+e.yxy, world).distance - get_closest(position-e.yxy, world).distance,
        get_closest(position+e.yyx, world).distance - get_closest(position-e.yyx, world).distance
    );
    return normalize(n);
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
    let circle = Primitive(TYPE_SPHERE, vec3<f32>(0.0), vec4<f32>(1.5), vec3<f32>(0.2, 0.3, 0.4));
    let box = Primitive(TYPE_BOX, vec3<f32>(0.0), vec4<f32>(0.7, 0.5, 10.8, 0.3), vec3<f32>(0.4, 0.3, 0.2));
    let other_circle = Primitive(TYPE_SPHERE, vec3<f32>(5.0, 10.0, -8.0), vec4<f32>(0.9), vec3<f32>(0.8, 0.2, 0.5));
    //let mandelbulb = Primitive(TYPE_MANDELBULB, vec3<f32>(5.0), vec4<f32>(0.0), vec3(0.3, 0.4, 0.2));

    var world = World(3, array<Primitive, 3>(box, circle, other_circle));

    // Trace
    var colour = mix(vec3<f32>(0.8, 0.9, 1.0), vec3<f32>(0.1, 0.3, 0.8), ray.direction.y * 0.5 + 0.5);
    var distance = 0.0;
    while (distance < 100.0) {
        let hit = get_closest(ray.position, world);

        if (hit.distance <= ETA) {
            let normal = get_normal(ray.position, world);
            let light_direction = normalize(vec3<f32>(1.0, 2.0, -1.0));
            let diffuse_intensity = max(dot(normal, light_direction), 0.0);
            let ambient = 0.1;

            // Shadow
            let bias = 0.005;
            var shadow_ray = Ray(ray.position + normal * bias, light_direction);

            var shadow_dist = bias;
            var un_shadow = 1.0;
            var ph = INFINITY;
            while (shadow_dist < 100.0) {
                let shadow_hit = get_closest(shadow_ray.position, world);
                let h = shadow_hit.distance;
                
                if (h <= ETA) {
                    un_shadow = 0.0;
                    break;
                }

                let y = (h*h) / (2.0*ph);
                let d = sqrt(max(0.0, h*h-y*y));

                un_shadow = min(un_shadow, 16.0 * d / max(ETA, shadow_dist - y));

                ph = h;

                shadow_dist += h;
                shadow_ray.position += shadow_ray.direction * h;
            }
            let direct = diffuse_intensity * un_shadow;
            let lighting = ambient + (1.0 - ambient) * direct;
            colour = hit.colour * lighting;

            break;
        }

        let delta = ray.direction * hit.distance;

        distance += hit.distance;

        ray.position += delta;
    }

    // Fill pixel

    textureStore(output, vec2<i32>(id.xy), vec4<f32>(colour, 1.0));
}
