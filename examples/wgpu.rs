use bytemuck::bytes_of;
use nohash::IntMap;
use std::{
    any::Any,
    borrow::Cow,
    path::Path,
    sync::{Arc, Mutex, OnceLock},
};
use ultralight::{
    gpu_driver::GpuDriver,
    sys::{
        ulApplyProjection, ulBitmapIsEmpty, ulViewGetRenderTarget,
        ULBitmapFormat_kBitmapFormat_A8_UNORM, ULBitmapFormat_kBitmapFormat_BGRA8_UNORM_SRGB,
        ULCommandType_kCommandType_ClearRenderBuffer, ULCommandType_kCommandType_DrawGeometry,
        ULShaderType_kShaderType_Fill, ULVertexBufferFormat,
        ULVertexBufferFormat_kVertexBufferFormat_2f_4ub_2f,
        ULVertexBufferFormat_kVertexBufferFormat_2f_4ub_2f_2f_28f, ULVertex_2f_4ub_2f,
        ULVertex_2f_4ub_2f_2f_28f,
    },
};
use wgpu::{
    util::DeviceExt, AddressMode, Backends, Color, ColorWrites, Dx12Compiler, Gles3MinorVersion,
    ImageSubresourceRange, InstanceDescriptor, InstanceFlags, RenderPipelineDescriptor,
    SamplerDescriptor, ShaderModule,
};
use winit::{
    dpi::{LogicalPosition, LogicalSize},
    event::{DeviceEvent, Event, WindowEvent},
    event_loop::EventLoop,
    window::Window,
};

fn encoder_pool() -> &'static Mutex<Vec<wgpu::CommandBuffer>> {
    static ARRAY: OnceLock<Mutex<Vec<wgpu::CommandBuffer>>> = OnceLock::new();
    ARRAY.get_or_init(|| Mutex::new(vec![]))
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct UniformBuffer {
    state: [f32; 4],
    transform: [f32; 16],
    scalar4: [f32; 8],
    vector: [[f32; 4]; 8],
    clip_size: u32,
    padding: [u32; 3],
    clip: [[f32; 16]; 8],
}

#[derive(Hash, Copy, Clone, PartialEq, Eq)]
struct PipelineDesc {}

struct WebGpuDriver {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,

    next_texture_id: u32,
    pub textures: IntMap<u32, (wgpu::Texture, wgpu::TextureView)>,

    next_render_buffer_id: u32,
    free_render_buffer_ids: Vec<u32>,
    pub render_buffers: IntMap<u32, ultralight::sys::ULRenderBuffer>,

    next_geometry_id: u32,
    geometries: IntMap<u32, (wgpu::Buffer, ULVertexBufferFormat, wgpu::Buffer)>,

    pipeline_cache: Vec<(wgpu::RenderPipeline, wgpu::BindGroup)>,

    fs_shader: ShaderModule,
    vs_shader: ShaderModule,
    fs_shader2: ShaderModule,
    vs_shader2: ShaderModule,
}

impl<'a> WebGpuDriver {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        let spirv_ps_fill_path = hassle_rs::compile_hlsl(
            "frag_fill.hlsl",
            include_str!("shaders/fragment_fill_path.glsl"),
            "main",
            "ps_5_1",
            &vec!["-spirv"],
            &vec![],
        )
        .unwrap();

        let spirv_vs_fill_path = hassle_rs::compile_hlsl(
            "frag_fill.hlsl",
            include_str!("shaders/vertex_fill_path.glsl"),
            "main",
            "vs_5_1",
            &vec!["-spirv"],
            &vec![],
        )
        .unwrap();

        let fs_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(bytemuck::cast_slice(
                &spirv_ps_fill_path,
            ))),
        });

        let vs_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(bytemuck::cast_slice(
                &spirv_vs_fill_path,
            ))),
        });

        let spirv_ps_fill = hassle_rs::compile_hlsl(
            "frag_fill.hlsl",
            include_str!("shaders/frag_fill.hlsl"),
            "main",
            "ps_5_1",
            &vec!["-spirv"],
            &vec![],
        )
        .unwrap();

        let spirv_vs_fill = hassle_rs::compile_hlsl(
            "frag_fill.hlsl",
            include_str!("shaders/vert_fill.hlsl"),
            "main",
            "vs_5_1",
            &vec!["-spirv"],
            &vec![],
        )
        .unwrap();

        let fs_shader2 = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(bytemuck::cast_slice(&spirv_ps_fill))),
        });

        let vs_shader2 = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(bytemuck::cast_slice(&spirv_vs_fill))),
        });

        Self {
            device,
            queue,
            next_texture_id: 1,
            next_render_buffer_id: 0,
            free_render_buffer_ids: vec![],
            next_geometry_id: 1,
            textures: Default::default(),
            render_buffers: Default::default(),
            geometries: Default::default(),
            pipeline_cache: Default::default(),
            fs_shader,
            vs_shader,
            fs_shader2,
            vs_shader2,
        }
    }
}

impl GpuDriver for WebGpuDriver {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn next_texture_id(&mut self) -> u32 {
        let next = self.next_texture_id;
        self.next_texture_id += 1;
        next
    }

    fn create_texture(&mut self, id: u32, bitmap: *mut ultralight::sys::C_Bitmap) {
        let format = unsafe { ultralight::sys::ulBitmapGetFormat(bitmap) };

        let texture_descriptor = wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: unsafe { ultralight::sys::ulBitmapGetWidth(bitmap) },
                height: unsafe { ultralight::sys::ulBitmapGetHeight(bitmap) },
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: if format == ULBitmapFormat_kBitmapFormat_A8_UNORM {
                wgpu::TextureFormat::Bgra8Unorm
            } else if format == ULBitmapFormat_kBitmapFormat_BGRA8_UNORM_SRGB {
                wgpu::TextureFormat::Bgra8UnormSrgb
            } else {
                unreachable!()
            },
            usage: if unsafe { ulBitmapIsEmpty(bitmap) } {
                wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
            } else {
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST
            },
            label: Some("ultralight texture"),
            view_formats: &[],
        };
        let texture = self.device.create_texture(&texture_descriptor);
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.textures.insert(id, (texture, texture_view));
        if unsafe { !ulBitmapIsEmpty(bitmap) } {
            self.update_texture(id, bitmap);
        }
    }

    fn update_texture(&mut self, id: u32, bitmap: *mut ultralight::sys::C_Bitmap) {
        let texture = &self.textures[&id].0;

        let bytes_per_pixel = unsafe { ultralight::sys::ulBitmapGetBpp(bitmap) };
        let width = unsafe { ultralight::sys::ulBitmapGetWidth(bitmap) };
        let height = unsafe { ultralight::sys::ulBitmapGetHeight(bitmap) };
        let mut bytes_per_row = unsafe { ultralight::sys::ulBitmapGetRowBytes(bitmap) };

        unsafe { ultralight::sys::ulBitmapLockPixels(bitmap) };
        let pixels_ptr = unsafe { ultralight::sys::ulBitmapRawPixels(bitmap) };
        let bitmap_data = unsafe {
            std::slice::from_raw_parts(pixels_ptr as _, (width * height * bytes_per_pixel) as usize)
        };

        // WGPU is trash and doesn't allow sampling R8 as alpha.
        let format = unsafe { ultralight::sys::ulBitmapGetFormat(bitmap) };
        let a8_converted = if format == ULBitmapFormat_kBitmapFormat_A8_UNORM as i32 {
            bytes_per_row = width * 4;
            bitmap_data
                .iter()
                .map(|v| [*v, *v, *v, *v])
                .flatten()
                .collect::<Vec<u8>>()
        } else {
            bytes_per_row = width * bytes_per_pixel;
            vec![]
        };

        assert!(width == texture.width() && height == texture.height());

        self.queue.write_texture(
            texture.as_image_copy(),
            if format == ULBitmapFormat_kBitmapFormat_A8_UNORM as i32 {
                &a8_converted
            } else {
                bitmap_data
            },
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        unsafe { ultralight::sys::ulBitmapUnlockPixels(bitmap) };
    }

    fn next_render_buffer_id(&mut self) -> u32 {
        if !self.free_render_buffer_ids.is_empty() {
            self.free_render_buffer_ids.swap_remove(0)
        } else {
            let next = self.next_render_buffer_id;
            self.next_render_buffer_id += 1;
            next
        }
    }

    fn create_render_buffer(&mut self, id: u32, render_buffer: ultralight::sys::ULRenderBuffer) {
        self.render_buffers.insert(id, render_buffer);
    }

    fn next_geometry_id(&mut self) -> u32 {
        let next = self.next_geometry_id;
        self.next_geometry_id += 1;
        next
    }

    fn create_geometry(
        &mut self,
        id: u32,
        vb: ultralight::sys::ULVertexBuffer,
        ib: ultralight::sys::ULIndexBuffer,
    ) {
        let vertex_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: unsafe { std::slice::from_raw_parts(vb.data, vb.size as usize) },
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: unsafe { std::slice::from_raw_parts(ib.data, ib.size as usize) },
                usage: wgpu::BufferUsages::INDEX,
            });

        self.geometries
            .insert(id, (vertex_buf, vb.format, index_buf));
    }

    fn update_geometry(
        &mut self,
        id: u32,
        vb: ultralight::sys::ULVertexBuffer,
        ib: ultralight::sys::ULIndexBuffer,
    ) {
        let geometry = self.geometries.get_mut(&id).unwrap();

        geometry.0 = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: unsafe { std::slice::from_raw_parts(vb.data, vb.size as usize) },
                usage: wgpu::BufferUsages::VERTEX,
            });
        geometry.1 = vb.format;
        geometry.2 = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: unsafe { std::slice::from_raw_parts(ib.data, ib.size as usize) },
                usage: wgpu::BufferUsages::INDEX,
            });
    }

    fn update_command_list(&mut self, cmd_list: ultralight::sys::ULCommandList) {
        let device = self.device.clone();

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let texture_descriptor = wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            label: None,
            view_formats: &[],
        };
        let fallback_texture = device.create_texture(&texture_descriptor);
        let fallbackview = fallback_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let fallback_texture = (fallback_texture, fallbackview);

        let cmds = unsafe { std::slice::from_raw_parts(cmd_list.commands, cmd_list.size as usize) };
        for cmd in cmds {
            let render_buffer = self.render_buffers[&cmd.gpu_state.render_buffer_id];
            let render_buffer_texture = &self.textures[&render_buffer.texture_id];

            if render_buffer.has_depth_buffer {
                println!("warning, depth buffer unsupported");
            }
            if render_buffer.has_stencil_buffer {
                println!("warning, depth buffer unsupported");
            }
            if cmd.gpu_state.enable_scissor {
                println!("warning, depth buffer unsupported");
            }

            assert!(
                render_buffer.texture_id != cmd.gpu_state.texture_1_id,
                "{}",
                render_buffer.texture_id
            );
            assert!(
                render_buffer.texture_id != cmd.gpu_state.texture_2_id,
                "{}",
                render_buffer.texture_id
            );
            assert!(
                render_buffer.texture_id != cmd.gpu_state.texture_3_id,
                "{}",
                render_buffer.texture_id
            );

            if cmd.command_type as i32 == ULCommandType_kCommandType_ClearRenderBuffer {
                encoder.clear_texture(&render_buffer_texture.0, &ImageSubresourceRange::default());
            } else if cmd.command_type as i32 == ULCommandType_kCommandType_DrawGeometry {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &render_buffer_texture.1,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                let (vb, vb_format, ib) = &self.geometries[&cmd.geometry_id];

                let vertex_buffer_layouts = [wgpu::VertexBufferLayout {
                    array_stride: if *vb_format
                        == ULVertexBufferFormat_kVertexBufferFormat_2f_4ub_2f
                    {
                        std::mem::size_of::<ULVertex_2f_4ub_2f>() as u64
                    } else if *vb_format
                        == ULVertexBufferFormat_kVertexBufferFormat_2f_4ub_2f_2f_28f
                    {
                        std::mem::size_of::<ULVertex_2f_4ub_2f_2f_28f>() as u64
                    } else {
                        unreachable!()
                    },
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: if *vb_format == ULVertexBufferFormat_kVertexBufferFormat_2f_4ub_2f
                    {
                        &[
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 0,
                                shader_location: 0,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Uint8x4,
                                offset: 2 * 4,
                                shader_location: 1,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 3 * 4,
                                shader_location: 2,
                            },
                        ]
                    } else if *vb_format
                        == ULVertexBufferFormat_kVertexBufferFormat_2f_4ub_2f_2f_28f
                    {
                        &[
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 0,
                                shader_location: 0,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Uint8x4,
                                offset: 2 * 4,
                                shader_location: 1,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 3 * 4,
                                shader_location: 2,
                            },
                            // texcoord
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 5 * 4,
                                shader_location: 3,
                            },
                            // data0
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 7 * 4,
                                shader_location: 4,
                            },
                            // data1
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 11 * 4,
                                shader_location: 5,
                            },
                            // data2
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 15 * 4,
                                shader_location: 6,
                            },
                            // data3
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 19 * 4,
                                shader_location: 7,
                            },
                            // data4
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 23 * 4,
                                shader_location: 8,
                            },
                            // data5
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 27 * 4,
                                shader_location: 9,
                            },
                            // data6
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 31 * 4,
                                shader_location: 10,
                            },
                        ]
                    } else {
                        unreachable!()
                    },
                }];

                // Create pipeline layout
                let bind_group_layout =
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: None,
                        entries: &[
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: wgpu::ShaderStages::VERTEX
                                    | wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Uniform,
                                    has_dynamic_offset: false,
                                    min_binding_size: wgpu::BufferSize::new(768),
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 1,
                                visibility: wgpu::ShaderStages::VERTEX
                                    | wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    multisampled: false,
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: true,
                                    },
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 2,
                                visibility: wgpu::ShaderStages::VERTEX
                                    | wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    multisampled: false,
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: true,
                                    },
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 3,
                                visibility: wgpu::ShaderStages::VERTEX
                                    | wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                                count: None,
                            },
                        ],
                    });
                let pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: None,
                        bind_group_layouts: &[&bind_group_layout],
                        push_constant_ranges: &[],
                    });

                let mvp = unsafe {
                    ulApplyProjection(
                        cmd.gpu_state.transform,
                        cmd.gpu_state.viewport_width as f32,
                        cmd.gpu_state.viewport_height as f32,
                        false,
                    )
                };

                let uniform_data = UniformBuffer {
                    state: [
                        0.0,
                        cmd.gpu_state.viewport_width as f32,
                        cmd.gpu_state.viewport_height as f32,
                        1.0,
                    ],
                    transform: mvp.data,
                    scalar4: cmd.gpu_state.uniform_scalar,
                    vector: cmd.gpu_state.uniform_vector.map(|i| i.value),
                    clip_size: cmd.gpu_state.clip_size as u32,
                    padding: [0; 3],
                    clip: cmd.gpu_state.clip.map(|v| v.data),
                };

                let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Uniform Buffer"),
                    contents: bytes_of(&uniform_data),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

                let tex1 = self
                    .textures
                    .get(&cmd.gpu_state.texture_1_id)
                    .unwrap_or(&fallback_texture);
                let tex2 = self
                    .textures
                    .get(&cmd.gpu_state.texture_2_id)
                    .unwrap_or(&fallback_texture);

                let mut test = SamplerDescriptor::default();
                test.address_mode_u = AddressMode::ClampToEdge;
                test.address_mode_v = AddressMode::ClampToEdge;
                test.address_mode_w = AddressMode::ClampToEdge;
                test.border_color = Some(wgpu::SamplerBorderColor::TransparentBlack);
                let sampler = device.create_sampler(&test);

                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: uniform_buf.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&tex1.1),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&tex2.1),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                    ],
                    label: None,
                });

                let blend = if cmd.gpu_state.enable_blend {
                    Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::OneMinusDstAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    })
                } else {
                    None
                };

                let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
                    label: None,
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: if cmd.gpu_state.shader_type == ULShaderType_kShaderType_Fill as u8
                        {
                            &self.vs_shader2
                        } else {
                            &self.vs_shader
                        },
                        entry_point: "main",
                        buffers: &vertex_buffer_layouts,
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: if cmd.gpu_state.shader_type == ULShaderType_kShaderType_Fill as u8
                        {
                            &self.fs_shader2
                        } else {
                            &self.fs_shader
                        },
                        entry_point: "main",
                        targets: &[Some(wgpu::ColorTargetState {
                            format: render_buffer_texture.0.format(),
                            blend,
                            write_mask: ColorWrites::all(),
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        cull_mode: None,
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        ..Default::default()
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                });

                self.pipeline_cache.push((pipeline, bind_group));
                let (pipeline, bind_group) = self.pipeline_cache.last().as_ref().unwrap();

                rpass.set_pipeline(pipeline);
                rpass.set_bind_group(0, bind_group, &[]);
                rpass.set_vertex_buffer(0, vb.slice(..));
                rpass.set_viewport(
                    0.0,
                    0.0,
                    cmd.gpu_state.viewport_width as f32,
                    cmd.gpu_state.viewport_height as f32,
                    0.0,
                    1.0,
                );
                rpass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);

                rpass.draw_indexed(
                    cmd.indices_offset..cmd.indices_offset + cmd.indices_count,
                    0,
                    0..1,
                )
            } else {
                unreachable!()
            }
        }

        encoder_pool().lock().unwrap().push(encoder.finish());
    }

    fn destroy_texture(&mut self, id: u32) {
        self.textures.remove_entry(&id);
    }

    fn destroy_render_buffer(&mut self, id: u32) {
        self.render_buffers.remove_entry(&id);
        self.free_render_buffer_ids.push(id);
    }

    fn destroy_geometry(&mut self, id: u32) {
        self.geometries.remove_entry(&id);
    }
}

async fn run(event_loop: EventLoop<()>, window: &Window) {
    let mut size = window.inner_size();
    size.width = size.width.max(1);
    size.height = size.height.max(1);

    let instance = wgpu::Instance::new(InstanceDescriptor {
        backends: Backends::VULKAN,
        flags: InstanceFlags::default(),
        dx12_shader_compiler: Dx12Compiler::Dxc {
            dxil_path: Some(
                Path::new("C:\\Users\\bideb\\ultralight\\target\\release\\dxil.dll").to_path_buf(),
            ),
            dxc_path: Some(
                Path::new("C:\\Users\\bideb\\ultralight\\target\\release\\dxcompiler.dll")
                    .to_path_buf(),
            ),
        },
        gles_minor_version: Gles3MinorVersion::default(),
    });

    let surface = instance.create_surface(window).unwrap();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            // Request an adapter which can render to our surface
            compatible_surface: Some(&surface),
        })
        .await
        .expect("Failed to find an appropriate adapter");

    // Create the logical device and command queue
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::CLEAR_TEXTURE
                    | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
                    | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
                // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                required_limits: wgpu::Limits::downlevel_defaults()
                    .using_resolution(adapter.limits()),
            },
            None,
        )
        .await
        .expect("Failed to create device");

    let device = Arc::new(device);
    let queue = Arc::new(queue);

    // Load the shaders from disk
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/triangle.wgsl"))),
    });

    let mut test = SamplerDescriptor::default();
    test.address_mode_u = AddressMode::ClampToEdge;
    test.address_mode_v = AddressMode::ClampToEdge;
    test.address_mode_w = AddressMode::ClampToEdge;
    test.border_color = Some(wgpu::SamplerBorderColor::TransparentBlack);
    let sampler = device.create_sampler(&test);

    // Create pipeline layout
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let swapchain_capabilities = surface.get_capabilities(&adapter);
    let swapchain_format = swapchain_capabilities.formats[0];

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(swapchain_format.into())],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    let mut config = surface
        .get_default_config(&adapter, size.width, size.height)
        .unwrap();
    surface.configure(&device, &config);

    let surface = Arc::new(surface);

    // Create driver
    let driver = Box::new(WebGpuDriver::new(device.clone(), queue.clone()));

    // Initialize ultralight
    ultralight::init("./examples/assets/".to_owned(), None);
    ultralight::gpu_driver::set_gpu_driver(driver);
    let mut ul_config = ultralight::Config::default();
    ul_config.set_resource_path_prefix("../resources/".to_owned());
    let mut renderer = ultralight::Renderer::new(&ul_config);
    let mut view_config = ultralight::ViewConfig::default();
    view_config.set_gpu_accelerated();
    let ul_view: ultralight::View = renderer.create_view(800, 600, &view_config);
    ul_view.load_url("file:///page.html".to_owned());

    let mut mouse_pos = LogicalPosition::new(0, 0);

    let window = &window;
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    event_loop
        .run(move |event, target| {
            // Have the closure take ownership of the resources.
            // `event_loop.run` never returns, therefore we must do this to ensure
            // the resources are properly cleaned up.
            let _ = (&instance, &adapter, &shader, &pipeline_layout);

            if let Event::DeviceEvent {
                device_id: _,
                event,
            } = event
            {
                match event {
                    DeviceEvent::Button { button: _, state } => match state {
                        winit::event::ElementState::Pressed => {
                            println!("{:?}", mouse_pos);
                            ul_view.mouse_pressed(mouse_pos.x, mouse_pos.y, true)
                        }
                        winit::event::ElementState::Released => {
                            ul_view.mouse_pressed(mouse_pos.x, mouse_pos.y, false)
                        }
                    },
                    _ => (),
                }
            } else if let Event::WindowEvent {
                window_id: _,
                event,
            } = event
            {
                match event {
                    WindowEvent::MouseWheel {
                        device_id: _,
                        delta,
                        phase: _,
                    } => match delta {
                        winit::event::MouseScrollDelta::LineDelta(x, y) => {
                            ul_view.mouse_scroll(x as i32, y as i32, true)
                        }
                        winit::event::MouseScrollDelta::PixelDelta(x) => {
                            let d = x.to_logical(window.scale_factor());
                            ul_view.mouse_scroll(d.x, d.y, false)
                        }
                    },
                    WindowEvent::CursorMoved {
                        device_id: _,
                        position,
                    } => {
                        let position = position.to_logical(window.scale_factor());
                        mouse_pos = position;
                        ul_view.mouse_moved(position.x, position.y)
                    }
                    WindowEvent::Resized(new_size) => {
                        // Reconfigure the surface with the new size
                        config.width = new_size.width.max(1);
                        config.height = new_size.height.max(1);
                        surface.configure(&device, &config);

                        if ul_view.is_ready() {
                            ul_view.resize(config.width, config.height);
                            ul_view.set_needs_repaint(true);
                        }

                        // On macos the window needs to be redrawn manually after resizing
                        window.request_redraw();
                    }
                    WindowEvent::RedrawRequested => {
                        renderer.update();

                        let frame = surface
                            .get_current_texture()
                            .expect("Failed to acquire next swap chain texture");

                        let view = frame
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());

                        renderer.render();

                        for encoder in encoder_pool().lock().unwrap().drain(..) {
                            queue.submit(Some(encoder));
                        }

                        let result = ultralight::gpu_driver::static_gpu_driver().lock().unwrap();
                        let result = result.as_ref().unwrap();
                        let result: &WebGpuDriver =
                            result.as_any().downcast_ref::<WebGpuDriver>().unwrap();
                        let rt = ul_view.get_render_target();
                        let texture = result.textures.get(&rt.texture_id);

                        let bind_group = if texture.is_some() {
                            device.create_bind_group(&wgpu::BindGroupDescriptor {
                                layout: &bind_group_layout,
                                entries: &[
                                    wgpu::BindGroupEntry {
                                        binding: 0,
                                        resource: wgpu::BindingResource::TextureView(
                                            &texture.unwrap().1,
                                        ),
                                    },
                                    wgpu::BindGroupEntry {
                                        binding: 1,
                                        resource: wgpu::BindingResource::Sampler(&sampler),
                                    },
                                ],
                                label: None,
                            })
                        } else {
                            let texture_descriptor = wgpu::TextureDescriptor {
                                size: wgpu::Extent3d {
                                    width: 1,
                                    height: 1,
                                    depth_or_array_layers: 1,
                                },
                                mip_level_count: 1,
                                sample_count: 1,
                                dimension: wgpu::TextureDimension::D2,
                                format: wgpu::TextureFormat::R8Unorm,
                                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                                label: None,
                                view_formats: &[],
                            };
                            let fallback_texture = device.create_texture(&texture_descriptor);
                            let fallbackview = fallback_texture
                                .create_view(&wgpu::TextureViewDescriptor::default());

                            device.create_bind_group(&wgpu::BindGroupDescriptor {
                                layout: &bind_group_layout,
                                entries: &[
                                    wgpu::BindGroupEntry {
                                        binding: 0,
                                        resource: wgpu::BindingResource::TextureView(&fallbackview),
                                    },
                                    wgpu::BindGroupEntry {
                                        binding: 1,
                                        resource: wgpu::BindingResource::Sampler(&sampler),
                                    },
                                ],
                                label: None,
                            })
                        };

                        let mut encoder =
                            device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: None,
                            });
                        {
                            let mut rpass =
                                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                    label: None,
                                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                        view: &view,
                                        resolve_target: None,
                                        ops: wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(Color::BLUE),
                                            store: wgpu::StoreOp::Store,
                                        },
                                    })],
                                    depth_stencil_attachment: None,
                                    timestamp_writes: None,
                                    occlusion_query_set: None,
                                });
                            rpass.set_bind_group(0, &bind_group, &[]);
                            rpass.set_pipeline(&render_pipeline);
                            rpass.draw(0..3, 0..1);
                        }
                        queue.submit(Some(encoder.finish()));

                        #[cfg(feature = "filewatching")]
                        if ultralight::platform::assets_modified() {
                            ul_view.reload();
                        }

                        frame.present();
                        window.request_redraw();
                    }
                    WindowEvent::CloseRequested => target.exit(),
                    _ => {}
                };
            }
        })
        .unwrap();
}

pub fn main() {
    let event_loop = EventLoop::new().unwrap();
    #[allow(unused_mut)]
    let mut builder = winit::window::WindowBuilder::new()
        .with_title("Ultralight WebGPU Driver")
        .with_inner_size(LogicalSize::new(800, 600));
    let window = builder.build(&event_loop).unwrap();

    env_logger::init();
    pollster::block_on(run(event_loop, &window));
}
