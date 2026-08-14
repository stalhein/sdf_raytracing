
use glam::*;

#[derive(Default)]
pub struct Camera {
    pub position: glam::Vec3,
    pub yaw: f32,
    pub pitch: f32,

    pub move_forward: bool,
    pub move_backward: bool,
    pub move_left: bool,
    pub move_right: bool,
    pub move_up: bool,
    pub move_down: bool,

    pub speed: f32,
    pub sensitivity: f32,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            position: glam::vec3(-2.0, 2.0, 0.0),
            yaw: 0.0,
            pitch: -0.3,
            move_forward: false,
            move_backward: false,
            move_left: false,
            move_right: false,
            move_up: false,
            move_down: false,
            speed: 5.0,
            sensitivity: 0.002,
        }
    }
    
    pub fn direction(&self) -> glam::Vec3 {
        glam::vec3(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos()
        ).normalize()
    }

    pub fn update(&mut self, dt: f32) {
        let forward_xz = glam::vec3(self.yaw.cos(), 0.0, self.yaw.sin()).normalize_or_zero();
        let right_xz = glam::vec3(-self.yaw.sin(), 0.0, self.yaw.cos()).normalize_or_zero();

        let mut move_direction = glam::Vec3::ZERO;

        if self.move_forward  { move_direction += forward_xz; }
        if self.move_backward { move_direction -= forward_xz; }
        if self.move_left     { move_direction -= right_xz; }
        if self.move_right    { move_direction += right_xz; }
        if self.move_up       { move_direction.y += 1.0; }
        if self.move_down     { move_direction.y -= 1.0; }

        if move_direction.length_squared() > 0.0 {
            self.position += move_direction.normalize() * self.speed * dt;
        }
    }

    pub fn process_mouse(&mut self, dx: f64, dy: f64) {
        self.yaw += (dx as f32) * self.sensitivity;
        self.pitch -= (dy as f32) * self.sensitivity;

        let max_pitch = 89.0_f32.to_radians();
        self.pitch = self.pitch.clamp(-max_pitch, max_pitch);
    }
}
