use std::{f64::consts::PI, iter, mem};

use cgmath::{abs_diff_ne, vec2, vec4, Matrix4, One, SquareMatrix, Vector2, Zero};
use wgpu::{util::DeviceExt, SurfaceConfiguration};
use winit::{
    dpi::{LogicalSize, PhysicalPosition},
    event::*,
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowBuilder},
};

mod game;
use game::render::*;
use game::*;

mod geometry;
use geometry::euclidian::*;
use geometry::hyperbolic::*;
use geometry::*;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x3];
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    transform: [[f32; 4]; 4],
}

impl Uniform {
    fn new() -> Self {
        Self {
            transform: Matrix4::identity().into(),
        }
    }
}

const SQRT2: f32 = 1.4142135623730951;
const STONE_VERTS: &[Vertex] = &[
    Vertex {
        position: [-STONE_RADIUS / SQRT2, STONE_RADIUS / SQRT2, 0.0],
    },
    Vertex {
        position: [-STONE_RADIUS, 0.0, 0.0],
    },
    Vertex {
        position: [-STONE_RADIUS / SQRT2, -STONE_RADIUS / SQRT2, 0.0],
    },
    Vertex {
        position: [0.0, -STONE_RADIUS, 0.0],
    },
    Vertex {
        position: [STONE_RADIUS / SQRT2, -STONE_RADIUS / SQRT2, 0.0],
    },
    Vertex {
        position: [STONE_RADIUS, 0.0, 0.0],
    },
    Vertex {
        position: [STONE_RADIUS / SQRT2, STONE_RADIUS / SQRT2, 0.0],
    },
    Vertex {
        position: [0.0, STONE_RADIUS, 0.0],
    },
];

const STONE_INDICES: &[u16] = &[0, 1, 2, 0, 2, 3, 0, 3, 4, 0, 4, 5, 0, 5, 6, 0, 6, 7];

const LINK_WIDTH: f32 = 0.06;
const LINK_VERTS: &[Vertex] = &[
    Vertex {
        position: [-LINK_WIDTH / 2.0, -LINK_WIDTH / 2.0, 0.0],
    },
    Vertex {
        position: [LINK_WIDTH / 2.0, -LINK_WIDTH / 2.0, 0.0],
    },
    Vertex {
        position: [-LINK_WIDTH / 2.0, 1.0 + LINK_WIDTH / 2.0, 0.0],
    },
    Vertex {
        position: [LINK_WIDTH / 2.0, 1.0 + LINK_WIDTH / 2.0, 0.0],
    },
];

const LINK_INDICES: &[u16] = &[0, 1, 2, 2, 1, 3];

struct InputState {
    forward: bool,
    left: bool,
    right: bool,
    back: bool,
}

impl InputState {
    fn new() -> Self {
        InputState {
            forward: false,
            left: false,
            right: false,
            back: false,
        }
    }

    fn process(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state,
                        physical_key: PhysicalKey::Code(keycode),
                        ..
                    },
                ..
            } => {
                let is_pressed = *state == ElementState::Pressed;
                match keycode {
                    KeyCode::KeyW | KeyCode::ArrowUp => {
                        self.forward = is_pressed;
                        true
                    }
                    KeyCode::KeyA | KeyCode::ArrowLeft => {
                        self.left = is_pressed;
                        true
                    }
                    KeyCode::KeyS | KeyCode::ArrowDown => {
                        self.back = is_pressed;
                        true
                    }
                    KeyCode::KeyD | KeyCode::ArrowRight => {
                        self.right = is_pressed;
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }
}

struct State<'a, SpinorT: Spinor> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,

    stone_vertex_buffer: wgpu::Buffer,
    stone_index_buffer: wgpu::Buffer,
    stone_instances: Vec<Instance>,
    stone_instance_buffer: wgpu::Buffer,

    link_vertex_buffer: wgpu::Buffer,
    link_index_buffer: wgpu::Buffer,
    link_instances: Vec<Instance>,
    link_instance_buffer: wgpu::Buffer,

    uniform: Uniform,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    window: &'a Window,

    input_state: InputState,
    cursor_pos: Vector2<f64>,
    view_state: ViewState<SpinorT>,
    game_state: GameState<SpinorT>,
    is_dragging: bool,
}

impl<'a, SpinorT: Spinor> State<'a, SpinorT> {
    async fn new(window: &'a Window) -> Self {
        let input_state = InputState::new();
        let view_state = ViewState::new();
        let game_state = GameState::new();

        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            //present_mode: surface_caps.present_modes[0],
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader.wgsl").into()),
        });

        let uniform = Uniform::new();
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniform_buffer"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("uniform_bind_group_layout"),
            });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("uniform_bind_group"),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render_pipeline_layout"),
                bind_group_layouts: &[&uniform_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render_pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc(), Instance::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let stone_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("stone_vertex_buffer"),
            contents: bytemuck::cast_slice(STONE_VERTS),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let stone_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("stone_index_buffer"),
            contents: bytemuck::cast_slice(STONE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        let stone_instances = Vec::new();
        let stone_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("stone_instance_buffer"),
            // TODO get size from game state?
            size: MAX_STONES * mem::size_of::<Instance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let link_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("link_vertex_buffer"),
            contents: bytemuck::cast_slice(LINK_VERTS),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let link_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("link_index_buffer"),
            contents: bytemuck::cast_slice(LINK_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        let link_instances = game_state.make_link_instances();
        let link_instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("link_instance_buffer"),
            contents: bytemuck::cast_slice(&link_instances),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            stone_vertex_buffer,
            stone_index_buffer,
            stone_instances,
            stone_instance_buffer,
            link_vertex_buffer,
            link_index_buffer,
            link_instances,
            link_instance_buffer,
            uniform,
            uniform_buffer,
            uniform_bind_group,
            window,
            input_state,
            cursor_pos: vec2(f64::NAN, f64::NAN),
            view_state,
            game_state,
            is_dragging: false,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn handle_mouse(&mut self, x: f64, y: f64) {
        //println!("handle_mouse {x} {y}");
        if self.is_dragging {
            // TODO
            //self.view_state
            //    .translate(self.view_state.pixel_delta_to_world(&self.config, x, y));
        }
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        if self.input_state.process(event) {
            return true;
        }
        match event {
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll_amt = match delta {
                    MouseScrollDelta::LineDelta(_, rows) => (*rows as f64) * 100.0,
                    MouseScrollDelta::PixelDelta(PhysicalPosition { y: scroll, .. }) => *scroll,
                };
                const SCROLL_FACTOR: f64 = 0.001;
                self.view_state.adjust_scale(scroll_amt * SCROLL_FACTOR);
                true
            }
            // TODO doesn't quite work when camera moves without cursor moving
            // can i just fetch cursor position and/or force update?
            WindowEvent::CursorMoved { position, .. } => {
                if !self.is_dragging {
                    self.cursor_pos =
                        self.view_state
                            .pixel_to_world_coords(&self.config, position.x, position.y);
                }
                true
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => {
                match *state {
                    ElementState::Pressed => self.game_state.select_point(self.cursor_pos),
                    ElementState::Released => (),
                }
                true
            }
            WindowEvent::MouseInput {
                button: MouseButton::Right,
                state,
                ..
            } => {
                match *state {
                    ElementState::Pressed => self.is_dragging = true,
                    ElementState::Released => self.is_dragging = false,
                }
                true
            }
            _ => false,
        }
    }

    fn update(&mut self) {
        const SPEED: f64 = 0.1;
        if self.input_state.back {
            self.view_state.translate(SPEED, 3.0 * PI / 2.0);
        } else if self.input_state.forward {
            self.view_state.translate(SPEED, PI / 2.0);
        }
        if self.input_state.left {
            self.view_state.translate(SPEED, PI);
        } else if self.input_state.right {
            self.view_state.translate(SPEED, 0.0);
        }
        self.uniform.transform = self.view_state.get_camera_mat().into();
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.uniform]),
        );

        if self.game_state.needs_render {
            self.stone_instances = self.game_state.make_stone_instances();
            self.queue.write_buffer(
                &self.stone_instance_buffer,
                0,
                bytemuck::cast_slice(&self.stone_instances[..]),
            );
            self.game_state.needs_render = false;
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.55,
                            g: 0.4,
                            b: 0.25,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.link_vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.link_instance_buffer.slice(..));
            render_pass
                .set_index_buffer(self.link_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(
                0..LINK_INDICES.len() as _,
                0,
                0..self.link_instances.len() as _,
            );

            render_pass.set_vertex_buffer(0, self.stone_vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.stone_instance_buffer.slice(..));
            render_pass
                .set_index_buffer(self.stone_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(
                0..STONE_INDICES.len() as _,
                0,
                0..self.stone_instances.len() as _,
            );
        }

        self.queue.submit(iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

pub async fn run() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_inner_size(LogicalSize {
            width: 1024,
            height: 1024,
        })
        .build(&event_loop)
        .unwrap();

    let mut state = State::<SpinorEuclidian>::new(&window).await;
    let mut surface_configured = false;

    event_loop
        .run(move |event, control_flow| match event {
            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta: (x, y) },
                ..
            } => state.handle_mouse(x, y),
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == state.window().id() => {
                if !state.input(event) {
                    match event {
                        WindowEvent::CloseRequested
                        | WindowEvent::KeyboardInput {
                            event:
                                KeyEvent {
                                    state: ElementState::Pressed,
                                    physical_key: PhysicalKey::Code(KeyCode::Escape),
                                    ..
                                },
                            ..
                        } => control_flow.exit(),
                        WindowEvent::Resized(physical_size) => {
                            surface_configured = true;
                            state.resize(*physical_size);
                            println!("resize event {:?}", *physical_size);
                        }
                        WindowEvent::RedrawRequested => {
                            state.window().request_redraw();

                            if !surface_configured {
                                return;
                            }

                            state.update();
                            match state.render() {
                                Ok(_) => {}
                                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                    state.resize(state.size)
                                }
                                Err(wgpu::SurfaceError::OutOfMemory) => {
                                    log::error!("OutOfMemory");
                                    control_flow.exit();
                                }
                                Err(wgpu::SurfaceError::Timeout) => {
                                    log::warn!("Surface timeout")
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        })
        .unwrap();
}

fn main() {
    pollster::block_on(run());
}
