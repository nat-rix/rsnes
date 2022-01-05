use clap::{ErrorKind, Parser};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use pollster::FutureExt;
use rsnes::{backend::ArrayFrameBuffer, device::Device, spc700::StereoSample};
use save_state::InSaveState;
use std::path::PathBuf;
use winit::{
    event::{DeviceEvent, ElementState, Event, KeyboardInput, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

#[derive(Parser, Clone)]
#[clap(
    version = clap::crate_version!(),
)]
struct Options {
    #[clap(parse(from_os_str))]
    input: PathBuf,
    #[clap(short, long)]
    verbose: bool,
}

macro_rules! error {
    ($($arg:tt)*) => {
        clap::app_from_crate!().error(ErrorKind::Io, format_args!($($arg)*)).exit()
    };
}

fn cartridge_from_file(path: &std::path::Path) -> rsnes::cartridge::Cartridge {
    let content = std::fs::read(path)
        .unwrap_or_else(|err| error!("Could not read file \"{}\" ({})\n", path.display(), err));
    rsnes::cartridge::Cartridge::from_bytes(&content).unwrap_or_else(|err| {
        error!(
            "Failure while reading cartridge file \"{}\" ({})\n",
            path.display(),
            err
        )
    })
}

struct AudioBackend {
    data: Arc<Mutex<Vec<i16>>>,
    _stream: cpal::platform::Stream,
    ahead: Arc<AtomicUsize>,
}

const SAMPLE_RATE: cpal::SampleRate = cpal::SampleRate(32000);

impl AudioBackend {
    fn new() -> Option<Self> {
        let host = cpal::default_host();
        let device = host.default_output_device()?;
        let cfg_range = device
            .supported_output_configs()
            .ok()?
            .filter(|cfg| {
                matches!(cfg.sample_format(), cpal::SampleFormat::I16)
                    && cfg.channels() == 2
                    && (cfg.min_sample_rate()..=cfg.max_sample_rate()).contains(&SAMPLE_RATE)
            })
            .min_by_key(|cfg| match cfg.buffer_size() {
                cpal::SupportedBufferSize::Unknown => cpal::FrameCount::MAX,
                cpal::SupportedBufferSize::Range { min, .. } => *min,
            })?;
        let cfg = cfg_range.with_sample_rate(SAMPLE_RATE).config();
        let data = Arc::new(Mutex::new(vec![]));
        let stream_data = Arc::clone(&data);
        let ahead = Arc::new(AtomicUsize::new(0));
        let ahead_ref = Arc::clone(&ahead);
        let stream = device
            .build_output_stream(
                &cfg,
                move |data: &mut [i16], _| {
                    if let Ok(mut sdata) = stream_data.lock() {
                        let size = data.len().min(sdata.len());
                        if size > 0 {
                            data[..size].copy_from_slice(&sdata.as_slice()[..size]);
                        }
                        if sdata.len() < data.len() {
                            data[sdata.len()..].fill(0)
                        }
                        if sdata.len() > data.len() {
                            sdata.copy_within(data.len().., 0);
                        }
                        let diff = sdata.len().max(data.len()) - data.len();
                        ahead_ref.store(diff, Ordering::Relaxed);
                        sdata.truncate(diff);
                    }
                },
                |_| (),
            )
            .ok()?;
        stream.play().ok()?;
        Some(Self {
            data,
            _stream: stream,
            ahead,
        })
    }
}

impl rsnes::backend::AudioBackend for AudioBackend {
    fn push_sample(&mut self, sample: StereoSample<i16>) {
        let mut lock = self.data.lock().unwrap();
        lock.push(sample.l);
        lock.push(sample.r);
    }
}

mod shaders {
    macro_rules! include_shader {
        ($t:expr) => {
            include_bytes!(concat!(env!("OUT_DIR"), "/", $t))
        };
    }

    static VERTEX_SHADER: &[u8] = include_shader!("main.vertex.spirv");
    static FRAGMENT_SHADER: &[u8] = include_shader!("main.fragment.spirv");

    fn create_shader(device: &wgpu::Device, source: &[u8]) -> wgpu::ShaderModule {
        device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: None, // TODO: label
            source: wgpu::util::make_spirv(source),
        })
    }

    static SHADER_ENTRY_POINT: &str = "main";

    pub fn create_vs(device: &wgpu::Device) -> (&str, wgpu::ShaderModule) {
        (SHADER_ENTRY_POINT, create_shader(device, VERTEX_SHADER))
    }

    pub fn create_fs(device: &wgpu::Device) -> (&str, wgpu::ShaderModule) {
        (SHADER_ENTRY_POINT, create_shader(device, FRAGMENT_SHADER))
    }
}

fn main() {
    let options = Options::parse();

    let cartridge = cartridge_from_file(&options.input);
    if options.verbose {
        println!(
            "[info] Cartridge header information: {:#?}",
            cartridge.header()
        );
    }
    let mut snes = Device::new(
        AudioBackend::new().unwrap_or_else(|| error!("Failed finding an audio output device")),
        ArrayFrameBuffer([[0; 4]; rsnes::backend::FRAME_BUFFER_SIZE], true),
    );
    snes.load_cartridge(cartridge);

    let size = winit::dpi::PhysicalSize::new(
        rsnes::ppu::SCREEN_WIDTH * 4,
        rsnes::ppu::MAX_SCREEN_HEIGHT * 4,
    );
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_decorations(true)
        .with_visible(true)
        .with_fullscreen(None)
        .with_resizable(false)
        .with_maximized(false)
        .with_inner_size(size)
        .with_title(env!("CARGO_PKG_NAME"))
        .build(&event_loop)
        .unwrap_or_else(|err| error!("Failure while creating window ({})", err));

    let inst = wgpu::Instance::new(wgpu::Backends::VULKAN);
    let surf = unsafe { inst.create_surface(&window) };
    let adapter = inst
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surf),
            force_fallback_adapter: false,
        })
        .block_on()
        .unwrap_or_else(|| error!("Failure finding a graphics adapter"));
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::empty(),
                limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            },
            None,
        )
        .block_on()
        .unwrap_or_else(|err| error!("Failure requesting a GPU command queue ({})", err));
    let (vs_entry, vs_shader) = shaders::create_vs(&device);
    let (fs_entry, fs_shader) = shaders::create_fs(&device);

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
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
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let texture_extent = wgpu::Extent3d {
        width: rsnes::ppu::SCREEN_WIDTH,
        height: rsnes::ppu::MAX_SCREEN_HEIGHT,
        depth_or_array_layers: 1,
    };
    let texture_format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: texture_extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: texture_format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    });
    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: None,
        format: Some(texture_format),
        dimension: Some(wgpu::TextureViewDimension::D2),
        aspect: wgpu::TextureAspect::All,
        base_mip_level: 0,
        mip_level_count: None,
        base_array_layer: 0,
        array_layer_count: None,
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: None,
        address_mode_u: wgpu::AddressMode::MirrorRepeat,
        address_mode_v: wgpu::AddressMode::MirrorRepeat,
        address_mode_w: wgpu::AddressMode::MirrorRepeat,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        lod_min_clamp: 100.0,
        lod_max_clamp: 100.0,
        compare: None,
        anisotropy_clamp: Some(core::num::NonZeroU8::new(1).unwrap()),
        border_color: None,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });

    let swapchain_format = surf.get_preferred_format(&adapter).unwrap();
    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &vs_shader,
            entry_point: vs_entry,
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &fs_shader,
            entry_point: fs_entry,
            targets: &[swapchain_format.into()],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });
    let mut surf_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: swapchain_format,
        width: size.width as u32,
        height: size.height as u32,
        present_mode: wgpu::PresentMode::Fifo,
    };
    surf.configure(&device, &surf_config);

    let mut last_rerendered = std::time::Instant::now();

    let mut shift = [false; 2];
    let mut savestates: [Option<Vec<u8>>; 10] = [(); 10].map(|()| None);

    event_loop.run(move |ev, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match ev {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::Resized(size) => {
                    surf_config.width = size.width;
                    surf_config.height = size.height;
                    surf.configure(&device, &surf_config);
                }
                _ => (),
            },
            Event::DeviceEvent { event, .. } => match event {
                DeviceEvent::Key(KeyboardInput {
                    scancode, state, ..
                }) => {
                    use rsnes::controller::buttons;
                    let key: u16 = match scancode {
                        0x24 => buttons::A,
                        0x25 => buttons::B,
                        0x26 => buttons::X,
                        0x27 => buttons::Y,
                        0x11 => buttons::UP,
                        0x1e => buttons::LEFT,
                        0x1f => buttons::DOWN,
                        0x20 => buttons::RIGHT,
                        0x10 => buttons::L,
                        0x12 => buttons::R,
                        0x38 => buttons::START,
                        0x64 => buttons::SELECT,
                        _ => {
                            match scancode {
                                0x2a => shift[0] = state == winit::event::ElementState::Pressed,
                                0x36 => shift[1] = state == winit::event::ElementState::Pressed,
                                2..=11 if state == winit::event::ElementState::Pressed => {
                                    let id = if scancode == 11 { 0 } else { scancode - 1 };
                                    let state = &mut savestates[id as usize];
                                    if shift[0] || shift[1] {
                                        if let Some(state) = state {
                                            // load save state
                                            let mut deserializer =
                                                save_state::SaveStateDeserializer {
                                                    data: state.iter(),
                                                };
                                            snes.deserialize(&mut deserializer);
                                        }
                                    } else {
                                        // store save state
                                        let mut serializer =
                                            save_state::SaveStateSerializer { data: vec![] };
                                        snes.serialize(&mut serializer);
                                        *state = Some(serializer.data);
                                    }
                                }
                                _ => (),
                            }
                            0
                        }
                    };
                    if key > 0 {
                        match &mut snes.controllers.port1.controller {
                            rsnes::controller::Controller::Standard(controller) => {
                                if let ElementState::Pressed = state {
                                    controller.pressed_buttons |= key
                                } else {
                                    controller.pressed_buttons &= !key
                                }
                            }
                            _ => (),
                        }
                    }
                }
                _ => (),
            },
            Event::MainEventsCleared => {
                if snes.spc.backend.ahead.load(Ordering::Relaxed) < 20_000 {
                    snes.run_cycle::<1>();
                    while !snes.new_frame {
                        snes.run_cycle::<1>();
                    }
                }
                let now = std::time::Instant::now();
                if last_rerendered + std::time::Duration::from_millis(16) <= now {
                    window.request_redraw();
                    last_rerendered = now;
                }
            }
            Event::RedrawRequested(_) => {
                match surf.get_current_texture() {
                    Ok(surface_texture) => {
                        if snes.ppu.frame_buffer.1 {
                            queue.write_texture(
                                texture.as_image_copy(),
                                snes.ppu.frame_buffer.get_bytes(),
                                wgpu::ImageDataLayout {
                                    offset: 0,
                                    bytes_per_row: core::num::NonZeroU32::new(
                                        4 * texture_extent.width,
                                    ),
                                    rows_per_image: core::num::NonZeroU32::new(
                                        texture_extent.height,
                                    ),
                                },
                                texture_extent,
                            );
                        }

                        let frame = &surface_texture.texture;
                        let view = frame.create_view(&wgpu::TextureViewDescriptor::default());
                        let mut encoder =
                            device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: None,
                            });
                        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: None,
                            color_attachments: &[wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Load,
                                    store: true,
                                },
                            }],
                            depth_stencil_attachment: None,
                        });
                        rpass.set_pipeline(&render_pipeline);
                        rpass.set_bind_group(0, &bind_group, &[]);
                        rpass.draw(0..6, 0..1);
                        drop(rpass);
                        queue.submit(Some(encoder.finish()));
                        surface_texture.present();
                    }
                    Err(wgpu::SurfaceError::Timeout) => {
                        eprintln!("[warning] surface acquire timeout");
                    }
                    Err(err) => error!("Failed to acquire next swap chain texture ({})", err),
                };
            }
            _ => (),
        }
    })
}
