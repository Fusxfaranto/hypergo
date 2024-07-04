use std::{f64::consts::PI, iter, mem};

use cgmath::{abs_diff_ne, vec2, vec4, Matrix4, One, SquareMatrix, Vector2, Zero};
use circular_buffer::CircularBuffer;
use clap::Parser;
use env_logger::{Builder, WriteStyle};
use log::{info, LevelFilter};
use web_time::Instant;
use wgpu::{util::DeviceExt, SurfaceConfiguration, TextureFormat};
use winit::{
    dpi::{LogicalSize, PhysicalPosition, PhysicalSize},
    event::*,
    event_loop::{self, EventLoop, EventLoopBuilder},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes},
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

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
    #[arg(long, default_value_t = 1 << 11)]
    internal_res: u32,
    #[arg(long, default_value_t = 4)]
    msaa: u32,
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
    w_scale: f32,
    h_scale: f32,
}

impl OuterUniform {
    fn new() -> Self {
        Self {
            f: 1.0,
            skip_reprojection: cfg!(feature = "euclidian_geometry") as u32,
            w_scale: 1.0,
            h_scale: 1.0,
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
                    KeyCode::KeyW => {
                        self.forward = is_pressed;
                        true
                    }
                    KeyCode::KeyA => {
                        self.left = is_pressed;
                        true
                    }
                    KeyCode::KeyS => {
                        self.back = is_pressed;
                        true
                    }
                    KeyCode::KeyD => {
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

fn limit_surface_res(size: PhysicalSize<u32>) -> PhysicalSize<u32> {
    const MAX_RES: u32 = if cfg!(target_arch = "wasm32") {
        1 << 11
    } else {
        1 << 15
    };

    let max_dim = size.width.max(size.height);

    if max_dim <= MAX_RES {
        size
    } else {
        PhysicalSize::<u32>::from_logical(
            size.to_logical::<f64>(1.0),
            MAX_RES as f64 / max_dim as f64,
        )
    }
}

struct TextRenderState {
    font_system: glyphon::FontSystem,
    swash_cache: glyphon::SwashCache,
    viewport: glyphon::Viewport,
    atlas: glyphon::TextAtlas,
    text_renderer: glyphon::TextRenderer,
    buffer_left: glyphon::Buffer,
    buffer_right: glyphon::Buffer,
}

impl TextRenderState {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, surface_format: TextureFormat) -> Self {
        let mut font_system = glyphon::FontSystem::new();
        let swash_cache = glyphon::SwashCache::new();
        let cache = glyphon::Cache::new(&device);
        let viewport = glyphon::Viewport::new(&device, &cache);
        let mut atlas = glyphon::TextAtlas::new(&device, &queue, &cache, surface_format);
        let text_renderer = glyphon::TextRenderer::new(
            &mut atlas,
            &device,
            wgpu::MultisampleState::default(),
            None,
        );
        let mut buffer_left =
            glyphon::Buffer::new(&mut font_system, glyphon::Metrics::new(30.0, 42.0));

        buffer_left.set_size(&mut font_system, 1000.0, 1000.0);
        buffer_left.shape_until_scroll(&mut font_system, false);

        let mut buffer_right =
            glyphon::Buffer::new(&mut font_system, glyphon::Metrics::new(30.0, 42.0));
        buffer_right.set_size(&mut font_system, 150.0, 150.0);
        buffer_right.shape_until_scroll(&mut font_system, false);

        TextRenderState {
            font_system,
            swash_cache,
            viewport,
            atlas,
            text_renderer,
            buffer_left,
            buffer_right,
        }
    }

    fn prepare(
        &mut self,
        text_left: &str,
        text_right: &str,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &wgpu::SurfaceConfiguration,
    ) -> Result<(), glyphon::PrepareError> {
        let attrs = glyphon::Attrs::new().family(glyphon::Family::SansSerif);
        self.buffer_left.set_text(
            &mut self.font_system,
            text_left,
            attrs,
            glyphon::Shaping::Advanced,
        );
        self.buffer_right.set_text(
            &mut self.font_system,
            text_right,
            attrs,
            glyphon::Shaping::Advanced,
        );
        // TODO doesn't seem to render anything when setting this?
        //self.buffer_right.lines[0].set_align(Some(glyphon::cosmic_text::Align::Right));

        self.viewport.update(
            &queue,
            glyphon::Resolution {
                width: config.width,
                height: config.height,
            },
        );

        self.text_renderer.prepare(
            &device,
            &queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            [
                glyphon::TextArea {
                    buffer: &self.buffer_left,
                    left: 10.0,
                    top: 10.0,
                    scale: 1.0,
                    bounds: glyphon::TextBounds::default(),
                    default_color: glyphon::Color::rgb(255, 255, 255),
                },
                glyphon::TextArea {
                    buffer: &self.buffer_right,
                    left: config.width as f32 - 150.0,
                    top: 10.0,
                    scale: 1.0,
                    bounds: glyphon::TextBounds::default(),
                    default_color: glyphon::Color::rgb(255, 255, 255),
                },
            ],
            &mut self.swash_cache,
        )
    }

    fn render<'pass>(
        &'pass self,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) -> Result<(), glyphon::RenderError> {
        self.text_renderer
            .render(&self.atlas, &self.viewport, render_pass)
    }

    // TODO this actually necessary?
    fn post_render(&mut self) {
        self.atlas.trim();
    }
}

struct State<'a, SpinorT: Spinor> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,

    text_render_state: TextRenderState,

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

    models: Vec<Model>,

    // TODO if these are going to continue using the same shader,
    // they should share gpu buffers
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

    frame_count: u64,
    last_frame_time: Instant,
    fps_ring: CircularBuffer<4, f64>,

    input_state: InputState,
    cursor_pos: SpinorT::Point,
    cursor_pos_clipped: bool,
    hover_point_pos_idx: Option<(SpinorT::Point, i32)>,
    view_state: ViewState<SpinorT>,
    game_state: GameState<SpinorT>,
    drag_from: Option<SpinorT::Point>,
    last_drag_pos: SpinorT::Point,
}

impl<'a, SpinorT: Spinor> State<'a, SpinorT> {
    async fn new(window: &'a Window) -> Self {
        let args = Args::parse();

        let ms_count = if cfg!(target_arch = "wasm32") {
            1
        } else {
            args.msaa
        };
        assert!(ms_count.count_ones() == 1);

        let input_state = InputState::new();
        let view_state = ViewState::new();
        let game_state = GameState::new();

        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
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
                    required_limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
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
        let surface_size = limit_surface_res(size);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: surface_size.width,
            height: surface_size.height,
            //present_mode: surface_caps.present_modes[0],
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        let text_render_state = TextRenderState::new(&device, &queue, surface_format);

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
                    count: ms_count,
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
            sample_count: ms_count,
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
                            multisampled: ms_count > 1,
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

        let outer_shader_src = if ms_count == 1 {
            concat!(
                include_str!("shaders/outer_shader_shared.wgsl"),
                include_str!("shaders/outer_shader_noms.wgsl")
            )
        } else {
            concat!(
                include_str!("shaders/outer_shader_shared.wgsl"),
                include_str!("shaders/outer_shader_ms.wgsl")
            )
        };
        let outer_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("outer_shader"),
            source: wgpu::ShaderSource::Wgsl(outer_shader_src.into()),
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
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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

        let models = make_models::<SpinorT>();
        info!("{:?}", models);
        //panic!();

        let stone_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("stone_vertex_buffer"),
            contents: bytemuck::cast_slice(&models[0].verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let stone_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("stone_index_buffer"),
            contents: bytemuck::cast_slice(&models[0].indices),
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
            contents: bytemuck::cast_slice(&models[1].verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let link_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("link_index_buffer"),
            contents: bytemuck::cast_slice(&models[1].indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let link_instances = game_state.make_link_instances();
        let link_instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("link_instance_buffer"),
            contents: bytemuck::cast_slice(&link_instances),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            surface,
            device,
            queue,
            config,
            size,
            text_render_state,
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
            models,
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
            frame_count: 0,
            last_frame_time: Instant::now(),
            fps_ring: CircularBuffer::<4, f64>::new(),
            input_state,
            cursor_pos: SpinorT::Point::zero(),
            cursor_pos_clipped: true,
            hover_point_pos_idx: None,
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
            let surface_size = limit_surface_res(new_size);
            self.config.width = surface_size.width;
            self.config.height = surface_size.height;
            self.surface.configure(&self.device, &self.config);

            // TODO skip for euclidian?
            let ar = surface_size.width as f64 / surface_size.height as f64;
            if ar > 1.0 {
                self.view_state.w_scale = 1.0 / ar;
                self.view_state.h_scale = 1.0;
            } else {
                self.view_state.w_scale = 1.0;
                self.view_state.h_scale = ar;
            }
            self.outer_uniform.w_scale = self.view_state.w_scale as f32;
            self.outer_uniform.h_scale = self.view_state.h_scale as f32;
            self.queue.write_buffer(
                &self.outer_uniform_buffer,
                0,
                bytemuck::cast_slice(&[self.outer_uniform]),
            )
        }
    }
    /*
    fn handle_mouse(&mut self, x: f64, y: f64) {
        //info!("handle_mouse {x} {y}");
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
                //info!("{:?}", position);
                (self.cursor_pos, self.cursor_pos_clipped) = self
                    .view_state
                    .pixel_to_world_coords(self.size, position.x, position.y);
                let last_hover_point_pos_idx = self.hover_point_pos_idx;
                let checking_pos = if self.cursor_pos_clipped {
                    None
                } else {
                    Some(self.cursor_pos)
                };
                self.hover_point_pos_idx = self.game_state.check_hover_point(checking_pos);
                if self.hover_point_pos_idx != last_hover_point_pos_idx {
                    self.game_state.needs_render = true;
                }
                true
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => {
                match *state {
                    ElementState::Pressed => {
                        if !self.cursor_pos_clipped {
                            self.game_state.select_point(self.cursor_pos)
                        }
                    }
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
                        physical_key: PhysicalKey::Code(keycode),
                        ..
                    },
                ..
            } => match keycode {
                KeyCode::KeyR => {
                    self.view_state.reset_camera();
                    true
                }
                KeyCode::ArrowLeft => {
                    self.game_state.move_history(-1);
                    true
                }
                KeyCode::ArrowRight => {
                    self.game_state.move_history(1);
                    true
                }
                KeyCode::KeyT => {
                    self.game_state.calculate_score();
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn update(&mut self) {
        self.frame_count += 1;
        const FPS_FAC: u64 = 10;
        if self.frame_count % FPS_FAC == 0 {
            self.fps_ring.push_back(
                (FPS_FAC as f64) / (Instant::now() - self.last_frame_time).as_secs_f64(),
            );

            self.last_frame_time = Instant::now();
        }

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

        if self
            .view_state
            .camera
            .distance(self.view_state.floating_origin)
            > 2.0
        {
            self.view_state.update_floating_origin();
            self.game_state
                .update_floating_origin(&self.view_state.camera.reverse());
        }

        self.uniform.transform = self.view_state.get_camera_mat().into();
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.uniform]),
        );

        if self.game_state.needs_render {
            self.link_instances = self.game_state.make_link_instances();
            self.queue.write_buffer(
                &self.link_instance_buffer,
                0,
                bytemuck::cast_slice(&self.link_instances[..]),
            );

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
            0..self.models[1].indices.len() as _,
            0,
            0..self.link_instances.len() as _,
        );

        render_pass.set_vertex_buffer(0, self.stone_vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.stone_instance_buffer.slice(..));
        render_pass.set_index_buffer(self.stone_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(
            0..self.models[0].indices.len() as _,
            0,
            0..self.stone_instances.len() as _,
        );
    }

    fn render_outer(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("outer_render_pass"),
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

        self.text_render_state.render(&mut render_pass).unwrap();
    }

    fn prepare_text(&mut self) -> Result<(), glyphon::PrepareError> {
        let mut avg_fps = 0.0;
        for &fps in self.fps_ring.iter() {
            avg_fps += fps;
        }
        avg_fps /= self.fps_ring.len() as f64;

        let camera_pos = self.view_state.camera.apply(SpinorT::Point::zero());
        // let floating_origin_pos = self
        //     .view_state
        //     .floating_origin
        //     .apply(SpinorT::Point::zero());

        let hover_display = if let Some((pos, idx)) = self.hover_point_pos_idx {
            format!("\nhovering over {:.2?} ({:})", pos, idx)
        } else {
            "".into()
        };

        let left_text = format!(
            "fps: {avg_fps:.2}\ncamera pos: {:.2?}{:}",
            camera_pos, hover_display
        );

        let score_display = if let Some(score) = &self.game_state.score {
            format!(
                "\nblack: {:}\nwhite: {:}",
                score.black_score, score.white_score
            )
        } else {
            "".into()
        };

        let right_text = format!(
            "turn {:}{:}",
            self.game_state.get_turn_count(),
            score_display
        );

        self.text_render_state.prepare(
            &left_text,
            &right_text,
            &self.device,
            &self.queue,
            &self.config,
        )
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.prepare_text().unwrap();

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
        self.render_outer(&mut encoder, &view);
        commands.push(encoder.finish());

        self.queue.submit(commands);
        output.present();

        self.text_render_state.post_render();

        Ok(())
    }
}

#[derive(Copy, Clone, Debug)]
struct CustomEvent {
    size: LogicalSize<u32>,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub async fn run() {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            console_log::init_with_level(log::Level::Info).expect("Couldn't initialize logger");
        } else {
            let mut log_builder = Builder::new();
            log_builder.filter(Some("hypergo"), LevelFilter::Info).write_style(WriteStyle::Always).init();
        }
    }

    let event_loop = EventLoop::<CustomEvent>::with_user_event().build().unwrap();
    let window = event_loop
        .create_window(
            WindowAttributes::default()
                .with_inner_size(LogicalSize {
                    width: 1024,
                    height: 1024,
                })
                .with_title("hypergo"),
        )
        .unwrap();

    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::closure::Closure;
        use winit::platform::web::WindowExtWebSys;

        let web_window = web_sys::window().unwrap();
        let dst = web_window
            .document()
            .unwrap()
            .get_element_by_id("game-render")
            .unwrap();
        let canvas = web_sys::Element::from(window.canvas().unwrap());
        dst.append_child(&canvas).ok().unwrap();

        const SIZE_HACK: u32 = 3;
        fn get_logical_size(web_window: &web_sys::Window) -> LogicalSize<u32> {
            let width = web_window.inner_width().unwrap().as_f64().unwrap() as u32;
            let height = web_window.inner_height().unwrap().as_f64().unwrap() as u32;
            let min_dim = width.min(height);
            LogicalSize::new(min_dim - SIZE_HACK, min_dim - SIZE_HACK)
        }

        let _ = window.request_inner_size(get_logical_size(&web_window));

        let event_loop_proxy = event_loop.create_proxy();
        let closure = Closure::wrap(Box::new(move |_web_event: web_sys::Event| {
            let web_window = web_sys::window().unwrap();
            event_loop_proxy
                .send_event(CustomEvent {
                    size: get_logical_size(&web_window),
                })
                .unwrap();
        }) as Box<dyn FnMut(_)>);
        web_window
            .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    #[cfg(feature = "euclidian_geometry")]
    use SpinorEuclidian as SpinorT;
    #[cfg(not(feature = "euclidian_geometry"))]
    use SpinorHyperbolic as SpinorT;

    let mut state = State::<SpinorT>::new(&window).await;
    let mut surface_configured = false;

    // TODO how is the non-deprecated version of this event loop supposed to work?
    event_loop
        .run(move |event, control_flow| match event {
            /*             Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta: (x, y) },
                ..
            } => state.handle_mouse(x, y), */
            Event::UserEvent(CustomEvent { size }) => {
                let _ = state.window.request_inner_size(size);
                info!("web resize event {:?}", size);
            }
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
                        WindowEvent::Resized(size) => {
                            surface_configured = true;
                            state.resize(*size);
                            info!("resize event {:?}", *size);
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
