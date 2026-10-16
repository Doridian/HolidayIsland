//! The GPU side: the framebuffer and palette as textures, drawn with one
//! fragment shader.

use std::fmt;
use std::sync::Arc;

use winit::dpi::PhysicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use crate::Frame;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Window(String),
    Gpu(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Window(e) => write!(f, "window: {e}"),
            Error::Gpu(e) => write!(f, "GPU: {e}"),
        }
    }
}

impl std::error::Error for Error {}

/// Where the framebuffer goes in the window. Matches `View` in the shader.
#[derive(Debug, Clone, Copy)]
pub(crate) struct View {
    pub offset: [f32; 2],
    pub scale: f32,
}

impl View {
    /// The largest whole scale that fits, centred; a fractional one if even
    /// 1:1 doesn't fit.
    #[allow(clippy::cast_precision_loss)]
    pub fn new(window: PhysicalSize<u32>, width: u32, height: u32) -> Self {
        let fit = (window.width as f32 / width.max(1) as f32).min(window.height as f32 / height.max(1) as f32);
        let scale = if fit >= 1.0 { fit.floor() } else { fit.max(f32::EPSILON) };
        let offset = [
            ((window.width as f32 - width as f32 * scale) / 2.0).floor(),
            ((window.height as f32 - height as f32 * scale) / 2.0).floor(),
        ];
        View { offset, scale }
    }

    fn bytes(self) -> [u8; 16] {
        let mut b = [0; 16];
        for (i, v) in [self.offset[0], self.offset[1], self.scale, 0.0]
            .into_iter()
            .enumerate()
        {
            b[4 * i..4 * i + 4].copy_from_slice(&v.to_le_bytes());
        }
        b
    }
}

struct Textures {
    width: u32,
    height: u32,
    framebuffer: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

pub(crate) struct Display {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    palette: wgpu::Texture,
    view: wgpu::Buffer,
    textures: Option<Textures>,
}

fn gpu(e: impl fmt::Display) -> Error {
    Error::Gpu(e.to_string())
}

impl Display {
    pub fn new(event_loop: &ActiveEventLoop, window: Arc<Window>) -> Result<Self, Error> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(
            event_loop.owned_display_handle(),
        )));
        let surface = instance.create_surface(window.clone()).map_err(gpu)?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(gpu)?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("holiday"),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .map_err(gpu)?;

        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| gpu("the surface doesn't fit the adapter"))?;
        // The shader writes the palette's sRGB values as they are.
        config.format = config.format.remove_srgb_suffix();
        config.view_formats = vec![config.format];
        surface.configure(&device, &config);

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                texture_entry(0, wgpu::TextureSampleType::Uint),
                texture_entry(1, wgpu::TextureSampleType::Float { filterable: false }),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(config.format.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        let palette = texture(&device, 256, 1, wgpu::TextureFormat::Rgba8Unorm);
        let view = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Display {
            window,
            surface,
            device,
            queue,
            config,
            pipeline,
            layout,
            palette,
            view,
            textures: None,
        })
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width > 0 && size.height > 0 {
            self.config.width = size.width;
            self.config.height = size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// Makes the framebuffer texture `width` × `height`.
    fn ensure_textures(&mut self, width: u32, height: u32) {
        if self
            .textures
            .as_ref()
            .is_none_or(|t| (t.width, t.height) != (width, height))
        {
            let framebuffer = texture(&self.device, width, height, wgpu::TextureFormat::R8Uint);
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &framebuffer.create_view(&wgpu::TextureViewDescriptor::default()),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(
                            &self.palette.create_view(&wgpu::TextureViewDescriptor::default()),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.view.as_entire_binding(),
                    },
                ],
            });
            self.textures = Some(Textures {
                width,
                height,
                framebuffer,
                bind_group,
            });
        }
    }

    pub fn draw(&mut self, frame: &Frame<'_>) -> Result<(), Error> {
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return Ok(()),
            other @ wgpu::CurrentSurfaceTexture::Validation => return Err(gpu(format!("{other:?}"))),
        };
        let size = PhysicalSize::new(self.config.width, self.config.height);
        let view = View::new(size, frame.width, frame.height);
        self.queue.write_buffer(&self.view, 0, &view.bytes());
        let rgba: Vec<u8> = frame.palette.iter().flat_map(|&[r, g, b]| [r, g, b, 255]).collect();
        write(&self.queue, &self.palette, &rgba, 256, 1, 4);
        self.ensure_textures(frame.width, frame.height);
        let Some(textures) = &self.textures else {
            return Ok(());
        };
        write(
            &self.queue,
            &textures.framebuffer,
            frame.pixels,
            frame.width,
            frame.height,
            1,
        );

        let target = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &textures.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        self.queue.present(output);
        Ok(())
    }
}

fn texture_entry(binding: u32, sample_type: wgpu::TextureSampleType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn texture(device: &wgpu::Device, width: u32, height: u32, format: wgpu::TextureFormat) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn write(queue: &wgpu::Queue, texture: &wgpu::Texture, data: &[u8], width: u32, height: u32, bytes_per_pixel: u32) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * bytes_per_pixel),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}
