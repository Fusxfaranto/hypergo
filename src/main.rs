use std::{f64::consts::PI, iter, mem, time};

use cgmath::{abs_diff_ne, vec2, vec4, Matrix4, One, SquareMatrix, Vector2, Zero};
use circular_buffer::CircularBuffer;
use clap::Parser;
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

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long, default_value_t = 1 << 13)]
    internal_res: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct RenderTargetVertex {
    position: [f32; 2],
}

impl RenderTargetVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x2];
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
struct OuterUniform {
    f: f32,
    skip_reprojection: u32,
}

impl OuterUniform {
    fn new() -> Self {
        Self {
            f: 1.0,
            skip_reprojection: cfg!(feature = "euclidian_geometry") as u32,
        }
    }
}

const RENDER_TARGET_VERTS: &[RenderTargetVertex] = &[
    RenderTargetVertex {
        position: [-1.0, -1.0],
    },
    RenderTargetVertex {
        position: [-1.0, 1.0],
    },
    RenderTargetVertex {
        position: [1.0, -1.0],
    },
    RenderTargetVertex {
        position: [-1.0, 1.0],
    },
    RenderTargetVertex {
        position: [1.0, -1.0],
    },
    RenderTargetVertex {
        position: [1.0, 1.0],
    },
];

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

const LINK_WIDTH: f32 = 0.025;
const LINK_VERTS: &[Vertex] = &[
    Vertex {
        position: [-LINK_WIDTH / 2.0, -LINK_WIDTH / 2.0, 0.0],
    },
    Vertex {
        position: [-LINK_WIDTH / 2.0, LINK_WIDTH / 2.0, 0.0],
    },
    Vertex {
        position: [1.0 + LINK_WIDTH / 2.0, -LINK_WIDTH / 2.0, 0.0],
    },
    Vertex {
        position: [1.0 + LINK_WIDTH / 2.0, LINK_WIDTH / 2.0, 0.0],
    },
];

const LINK_INDICES: &[u16] = &[0, 2, 1, 1, 2, 3];

struct InputState {
    forward: bool,
    left: bool,
    right: bool,
    back: bool,
    cw: bool,
    ccw: bool,
}

impl InputState {
    fn new() -> Self {
        InputState {
            forward: false,
            left: false,
            right: false,
            back: false,
            cw: false,
            ccw: false,
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
                    KeyCode::KeyE => {
                        self.cw = is_pressed;
                        true
                    }
                    KeyCode::KeyQ => {
                        self.ccw = is_pressed;
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

    outer_render_pipeline: wgpu::RenderPipeline,
    outer_uniform: OuterUniform,
    outer_uniform_buffer: wgpu::Buffer,
    outer_uniform_bind_group: wgpu::BindGroup,

    render_target_vertex_buffer: wgpu::Buffer,
    render_target_pipeline: wgpu::RenderPipeline,
    render_target_tex: wgpu::Texture,
    render_target_tex_view: wgpu::TextureView,
    render_target_tex_sampler: wgpu::Sampler,
    render_target_tex_bind_group: wgpu::BindGroup,

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
    cursor_pos: SpinorT::Point,
    view_state: ViewState<SpinorT>,
    game_state: GameState<SpinorT>,
    drag_from: Option<SpinorT::Point>,
    last_drag_pos: SpinorT::Point,
}

impl<'a, SpinorT: Spinor> State<'a, SpinorT> {
    async fn new(window: &'a Window) -> Self {
        let args = Args::parse();

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
                    // TODO presumably this can be made optional?
                    //required_features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
                    required_features: wgpu::Features::default(),
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

        const MULTISAMPLE_COUNT: u32 = 4;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader.wgsl").into()),
        });
        let render_target_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render_pipeline_layout"),
                bind_group_layouts: &[&uniform_bind_group_layout],
                push_constant_ranges: &[],
            });
        let render_target_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("render_target_pipeline"),
                layout: Some(&render_target_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    compilation_options: Default::default(),
                    buffers: &[Vertex::desc(), Instance::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent::OVER,
                            alpha: wgpu::BlendComponent::OVER,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: MULTISAMPLE_COUNT,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
            });
        let render_target_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render_target_tex"),
            // TODO pick a resolution more smartly
            size: wgpu::Extent3d {
                width: args.internal_res,
                height: args.internal_res,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: MULTISAMPLE_COUNT,
            dimension: wgpu::TextureDimension::D2,
            format: surface_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let render_target_tex_view =
            render_target_tex.create_view(&wgpu::TextureViewDescriptor::default());
        // TODO remove?
        let render_target_tex_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        // maybe some day try this again, but seems like i'd have to fork this to get it
        // to work with this rendering pipeline
        /*
        let render_target_smaa_target = smaa::SmaaTarget::new(
            &device,
            &queue,
            render_target_tex.width(),
            render_target_tex.height(),
            surface_format,
            smaa::SmaaMode::Smaa1X,
        ); */
        let render_target_tex_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: true,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("render_target_tex_bind_group_layout"),
            });
        let render_target_tex_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &render_target_tex_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&render_target_tex_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&render_target_tex_sampler),
                },
            ],
            label: Some("render_target_tex_bind_group"),
        });

        let outer_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("outer_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/outer_shader.wgsl").into()),
        });
        let outer_uniform = OuterUniform::new();
        let outer_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("outer_uniform_buffer"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let outer_uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("outer_uniform_bind_group_layout"),
            });
        let outer_uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &outer_uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: outer_uniform_buffer.as_entire_binding(),
            }],
            label: Some("outer_uniform_bind_group"),
        });
        let outer_render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("outer_render_pipeline_layout"),
                bind_group_layouts: &[
                    &render_target_tex_bind_group_layout,
                    &outer_uniform_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });
        let outer_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("outer_render_pipeline"),
                layout: Some(&outer_render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &outer_shader,
                    entry_point: "vs_main",
                    compilation_options: Default::default(),
                    buffers: &[RenderTargetVertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &outer_shader,
                    entry_point: "fs_main",
                    compilation_options: Default::default(),
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
                    cull_mode: None,
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

        let render_target_vertex_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("render_target_vertex_buffer"),
                contents: bytemuck::cast_slice(RENDER_TARGET_VERTS),
                usage: wgpu::BufferUsages::VERTEX,
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
            outer_render_pipeline,
            outer_uniform,
            outer_uniform_buffer,
            outer_uniform_bind_group,
            render_target_vertex_buffer,
            render_target_pipeline,
            render_target_tex,
            render_target_tex_view,
            render_target_tex_sampler,
            render_target_tex_bind_group,
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
            cursor_pos: SpinorT::Point::zero(),
            view_state,
            game_state,
            drag_from: None,
            last_drag_pos: SpinorT::Point::zero(),
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
    /*
    fn handle_mouse(&mut self, x: f64, y: f64) {
        //println!("handle_mouse {x} {y}");
        if self.is_dragging {
            // TODO
            //self.view_state
            //    .translate(self.view_state.pixel_delta_to_world(&self.config, x, y));
        }
    } */

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
                self.view_state
                    .adjust_projection_factor(scroll_amt * SCROLL_FACTOR);
                true
            }
            // TODO doesn't quite work when camera moves without cursor moving
            // can i just fetch cursor position and/or force update?
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos =
                    self.view_state
                        .pixel_to_world_coords(&self.config, position.x, position.y);
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
                    ElementState::Pressed => {
                        self.drag_from = Some(self.cursor_pos);
                    }
                    ElementState::Released => {
                        self.drag_from = None;
                    }
                }
                true
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(KeyCode::KeyR),
                        ..
                    },
                ..
            } => {
                self.view_state.reset_camera();
                true
            }
            /*             WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(KeyCode::KeyT),
                        ..
                    },
                ..
            } => {
                self.view_state.camera = SpinorT::new(
                    1.3090169943749472,
                    -0.42532540417601966,
                    0.5558929702514209,
                    -0.7651210339710757,
                );
                true
            } */
            _ => false,
        }
    }

    fn update(&mut self) {
        const SPEED: f64 = 0.1;
        if self.input_state.back {
            self.view_state.translate(SPEED, PI);
        } else if self.input_state.forward {
            self.view_state.translate(SPEED, 0.0);
        }
        if self.input_state.left {
            self.view_state.translate(SPEED, PI / 2.0);
        } else if self.input_state.right {
            self.view_state.translate(SPEED, 3.0 * PI / 2.0);
        }
        const ANGULAR_SPEED: f64 = 0.05;
        if self.input_state.cw {
            self.view_state.rotate(ANGULAR_SPEED);
        } else if self.input_state.ccw {
            self.view_state.rotate(-ANGULAR_SPEED);
        }

        if let Some(pos) = self.drag_from {
            if self.last_drag_pos != self.cursor_pos {
                self.view_state.drag(pos, self.cursor_pos);
                self.last_drag_pos = self.cursor_pos;
            }
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

        // TODO don't need to be updating this every frame
        self.outer_uniform.f = self.view_state.projection_factor as f32;
        self.queue.write_buffer(
            &self.outer_uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.outer_uniform]),
        )
    }

    fn render_to_render_target(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render_target_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.render_target_tex_view,
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

        render_pass.set_pipeline(&self.render_target_pipeline);
        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.link_vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.link_instance_buffer.slice(..));
        render_pass.set_index_buffer(self.link_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(
            0..LINK_INDICES.len() as _,
            0,
            0..self.link_instances.len() as _,
        );

        render_pass.set_vertex_buffer(0, self.stone_vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.stone_instance_buffer.slice(..));
        render_pass.set_index_buffer(self.stone_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(
            0..STONE_INDICES.len() as _,
            0,
            0..self.stone_instances.len() as _,
        );
    }

    fn render_transformed(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.2,
                        g: 0.2,
                        b: 0.2,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        render_pass.set_pipeline(&self.outer_render_pipeline);
        render_pass.set_bind_group(0, &self.render_target_tex_bind_group, &[]);
        render_pass.set_bind_group(1, &self.outer_uniform_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.render_target_vertex_buffer.slice(..));
        render_pass.draw(0..RENDER_TARGET_VERTS.len() as _, 0..1);
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // TODO can/should this be reused?
        let mut render_target_encoder =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render_target_encoder"),
                });
        self.render_to_render_target(&mut render_target_encoder);
        let mut commands = vec![render_target_encoder.finish()];

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encoder"),
            });
        self.render_transformed(&mut encoder, &view);
        commands.push(encoder.finish());

        self.queue.submit(commands);
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
        .with_title("hypergo")
        .build(&event_loop)
        .unwrap();

    #[cfg(feature = "euclidian_geometry")]
    use SpinorEuclidian as SpinorT;
    #[cfg(not(feature = "euclidian_geometry"))]
    use SpinorHyperbolic as SpinorT;

    let mut state = State::<SpinorT>::new(&window).await;
    let mut surface_configured = false;

    let mut frame_count = 0;
    let mut last_frame_time = time::Instant::now();
    let mut fps_ring = CircularBuffer::<4, f64>::new();

    event_loop
        .run(move |event, control_flow| match event {
            /*             Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta: (x, y) },
                ..
            } => state.handle_mouse(x, y), */
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
                            frame_count += 1;
                            const FPS_FAC: i32 = 10;
                            if frame_count % FPS_FAC == 0 {
                                fps_ring.push_back(
                                    (FPS_FAC as f64)
                                        / (time::Instant::now() - last_frame_time).as_secs_f64(),
                                );

                                last_frame_time = time::Instant::now();
                            }

                            if frame_count % 60 == 0 {
                                let mut avg_fps = 0.0;
                                for &fps in fps_ring.iter() {
                                    avg_fps += fps;
                                }
                                avg_fps /= fps_ring.len() as f64;
                                // TODO text rendering
                                if frame_count < 600 {
                                    println!("fps: {avg_fps}");
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
