
mod camera;
use crate::camera::Camera;
use bytemuck::{Pod, Zeroable};
use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use wgpu::Instance;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct CameraUniform {
    position: [f32; 3],
    _pad0: f32,
    direction: [f32; 3],
    _pad1: f32,
}

#[derive(Default)]
struct App {
    camera: Camera,
    last_frame: Option<Instant>,
    is_cursor_grabbed: bool,
    surface: Option<wgpu::Surface<'static>>,
    window: Option<Arc<Window>>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    config: Option<wgpu::SurfaceConfiguration>,
    texture: Option<wgpu::Texture>,
    texture_view: Option<wgpu::TextureView>,
    camera_buffer: Option<wgpu::Buffer>,
    compute_bind_group_layout: Option<wgpu::BindGroupLayout>,
    display_bind_group_layout: Option<wgpu::BindGroupLayout>,
    compute_pipeline: Option<wgpu::ComputePipeline>,
    display_pipeline: Option<wgpu::RenderPipeline>,
    compute_bind_group: Option<wgpu::BindGroup>,
    display_bind_group: Option<wgpu::BindGroup>,
}

impl App {
    fn create_texture_resources(&mut self) {
        // Return if no device and config
        let (device, config, camera_buffer) = match (&self.device, &self.config, &self.camera_buffer) {
            (Some(d), Some(c), Some(cb)) => (d, c, cb),
            _ => return,
        };

        let compute_layout = self.compute_bind_group_layout.as_ref().unwrap();
        let display_layout = self.display_bind_group_layout.as_ref().unwrap();

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Compute Texture"),
            size: wgpu::Extent3d {
                width: config.width.max(1),
                height: config.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Bind Group"),
            layout: compute_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: camera_buffer.as_entire_binding(),
                },
            ],
        });

        let display_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Display Bind Group"),
            layout: display_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            }],
        });

        self.texture = Some(texture);
        self.texture_view = Some(texture_view);
        self.compute_bind_group = Some(compute_bind_group);
        self.display_bind_group = Some(display_bind_group);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(event_loop
            .create_window(Window::default_attributes())
            .expect("Failed to create window."));

        let instance = Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).expect("Failed to create surface.");

        let adapter = pollster::block_on(
            instance.request_adapter(
                &wgpu::RequestAdapterOptions {
                    compatible_surface: Some(&surface),
                    ..Default::default()
                },
            ),
        ).expect("Failed to get adapter.");

        let (device, queue) = pollster::block_on(
            adapter.request_device(
                &wgpu::DeviceDescriptor::default(),
            ),
        ).expect("Failed to get device or queue.");

        let capabilities = surface.get_capabilities(&adapter);

        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(capabilities.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: window.inner_size().width,
            height: window.inner_size().height,
            present_mode: capabilities.present_modes[0],
            alpha_mode: capabilities.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);


        // Camera
        use wgpu::util::DeviceExt;
        let camera_uniform = CameraUniform {
            position: self.camera.position.to_array(),
            _pad0: 0.0,
            direction: self.camera.direction().to_array(),
            _pad1: 0.0,
        };

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Uniform Buffer"),
            contents: bytemuck::bytes_of(&camera_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });


        // Bind Groups
        let compute_bind_group_layout = 
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Compute Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            }
        );

        let display_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Display Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float {
                                filterable: false,
                            },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            },
        );


        // Pipelines
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let display_shader = device.create_shader_module(
            wgpu::ShaderModuleDescriptor {
                label: Some("Display Shader"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("display.wgsl").into()
                ),
            },
        );

        let compute_pipeline_layout = 
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Compute Pipeline Layout"),
                bind_group_layouts: &[&compute_bind_group_layout],
                push_constant_ranges: &[],
            }
        );

        let display_pipeline_layout = 
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Display Pipeline Layout"),
                bind_group_layouts: &[&display_bind_group_layout],
                push_constant_ranges: &[],
            }
        );

        let compute_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Compute Pipeline"),
                layout: Some(&compute_pipeline_layout),
                module: &compute_shader,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            }
        );

        let display_pipeline = device.create_render_pipeline(
            &wgpu::RenderPipelineDescriptor {
                label: Some("Display Pipeline"),
                layout: Some(&display_pipeline_layout),

                vertex: wgpu::VertexState {
                    module: &display_shader,
                    entry_point: Some("vertex"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },

                fragment: Some(wgpu::FragmentState {
                    module: &display_shader,
                    entry_point: Some("fragment"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),

                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            },
        );

        let camera = Camera::new();
        
        self.camera = camera;
        self.last_frame = Some(Instant::now());
        self.is_cursor_grabbed = false;
        self.window = Some(window);
        self.surface = Some(surface);
        self.device = Some(device);
        self.queue = Some(queue);
        self.config = Some(config);
        self.camera_buffer = Some(camera_buffer);
        self.compute_bind_group_layout = Some(compute_bind_group_layout);
        self.display_bind_group_layout = Some(display_bind_group_layout);
        self.compute_pipeline = Some(compute_pipeline);
        self.display_pipeline = Some(display_pipeline);

        self.create_texture_resources();

        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::MouseInput { state: winit::event::ElementState::Pressed,
                                      button: winit::event::MouseButton::Left, .. } => {
                if let Some(window) = &self.window {
                    window.set_cursor_visible(false);
                    let _ = window.set_cursor_grab(winit::window::CursorGrabMode::Locked)
                        .or_else(|_| window.set_cursor_grab(winit::window::CursorGrabMode::Confined));
                    self.is_cursor_grabbed = true;
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let is_pressed = event.state.is_pressed();

                if event.physical_key == KeyCode::Escape && is_pressed {
                    if let Some(window) = &self.window {
                        window.set_cursor_visible(true);
                        let _ = window.set_cursor_grab(winit::window::CursorGrabMode::None);
                        self.is_cursor_grabbed = false;
                    }
                }

                match event.physical_key {
                    PhysicalKey::Code(KeyCode::KeyW) => self.camera.move_forward = is_pressed,
                    PhysicalKey::Code(KeyCode::KeyS) => self.camera.move_backward = is_pressed,
                    PhysicalKey::Code(KeyCode::KeyA) => self.camera.move_left = is_pressed,
                    PhysicalKey::Code(KeyCode::KeyD) => self.camera.move_right = is_pressed,
                    PhysicalKey::Code(KeyCode::Space) => self.camera.move_up = is_pressed,
                    PhysicalKey::Code(KeyCode::ShiftLeft) => self.camera.move_down = is_pressed,
                    _ => {}
                }
            }
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    if let (Some(surface), Some(device), Some(config)) = 
                        (&self.surface, &self.device, &mut self.config)
                    {
                        config.width = size.width;
                        config.height = size.height;
                        surface.configure(device, config);

                        self.create_texture_resources();
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                // Update
                let now = Instant::now();
                if let Some(last) = self.last_frame {
                    let dt = now.duration_since(last).as_secs_f32();
                    self.camera.update(dt);
                }
                self.last_frame = Some(now);

                if let (Some(queue), Some(camera_buffer)) = (&self.queue, &self.camera_buffer) {
                    let camera_uniform = CameraUniform {
                        position: self.camera.position.to_array(),
                        _pad0: 0.0,
                        direction: self.camera.direction().to_array(),
                        _pad1: 0.0,
                    };
                    queue.write_buffer(camera_buffer, 0, bytemuck::bytes_of(&camera_uniform));
                }

                // Render
                let (surface, device, queue, config, compute_pipeline, display_pipeline, compute_bind_group, display_bind_group) =
                    match (&self.surface, &self.device, &self.queue, &self.config, &self.compute_pipeline,
                            &self.display_pipeline, &self.compute_bind_group, &self.display_bind_group) {
                    (
                        Some(s),
                        Some(d),
                        Some(q),
                        Some(c),
                        Some(cp),
                        Some(dp),
                        Some(cbg),
                        Some(dbg),
                    ) => (s, d, q, c, cp, dp, cbg, dbg),
                    _ => return,
                };

                let output = match surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(wgpu::SurfaceError::Outdated) => {
                        surface.configure(device, config);
                        return;
                    }
                    Err(wgpu::SurfaceError::Timeout) => return,
                    Err(e) => {
                        eprintln!("Surface error: {e:?}.");
                        return;
                    }
                };

                let output_view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Encoder") });

                // Compute pass
                {
                    let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some("Compute Pass"),
                        timestamp_writes: None,
                    });

                    compute_pass.set_pipeline(compute_pipeline);
                    compute_pass.set_bind_group(0, compute_bind_group, &[]);
                    compute_pass.dispatch_workgroups(
                        (config.width + 7) / 8,
                        (config.height + 7) / 8,
                        1,
                    );
                }

                // Display pass 
                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Display Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &output_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    
                    render_pass.set_pipeline(display_pipeline);
                    render_pass.set_bind_group(0, display_bind_group, &[]);
                    render_pass.draw(0..3, 0..1);
                }

                queue.submit(Some(encoder.finish()));
                output.present();

                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }   
            _ => (),
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        if let winit::event::DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            if self.is_cursor_grabbed {
                self.camera.process_mouse(dx, dy);
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;

    let mut app = App::default();

    event_loop.run_app(&mut app)?;

    Ok(())
}
