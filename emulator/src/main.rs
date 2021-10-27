use clap::{ErrorKind, Parser};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rsnes::{device::Device, spc700::StereoSample};
use std::path::PathBuf;
use winit::{
    event::{Event, WindowEvent},
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

fn error<E: std::fmt::Display>(kind: ErrorKind, val: E) -> ! {
    clap::app_from_crate!().error(kind, val).exit()
}

fn cartridge_from_file(path: &std::path::Path) -> rsnes::cartridge::Cartridge {
    let content = std::fs::read(path).unwrap_or_else(|err| {
        error(
            clap::ErrorKind::Io,
            format_args!("Could not read file \"{}\" ({})\n", path.display(), err),
        )
    });
    rsnes::cartridge::Cartridge::from_bytes(&content).unwrap_or_else(|err| {
        error(
            clap::ErrorKind::InvalidValue,
            format_args!(
                "Failure while reading cartridge file \"{}\" ({})\n",
                path.display(),
                err
            ),
        )
    })
}

struct EmulatorBackend;

impl rsnes::backend::Backend for EmulatorBackend {
    type Audio = AudioBackend;
    type Picture = rsnes::backend::PictureDummy;
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
            .next()?;
        let cfg = cfg_range.with_sample_rate(SAMPLE_RATE).config();
        let data = Arc::new(Mutex::new(vec![]));
        let stream_data = Arc::clone(&data);
        let ahead = Arc::new(AtomicUsize::new(0));
        let ahead_ref = Arc::clone(&ahead);
        let stream = device
            .build_output_stream(
                &cfg,
                move |data: &mut [i16], _| loop {
                    if let Ok(mut sdata) = stream_data.lock() {
                        if sdata.len() >= data.len() {
                            data.copy_from_slice(&sdata.as_slice()[..data.len()]);
                            sdata.copy_within(data.len().., 0);
                            let diff = sdata.len() - data.len();
                            ahead_ref.store(diff, Ordering::Relaxed);
                            sdata.truncate(diff);
                            break;
                        }
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

fn main() {
    let options = Options::parse();

    let cartridge = cartridge_from_file(&options.input);
    if options.verbose {
        println!(
            "[info] Cartridge header information: {:#?}",
            cartridge.header()
        );
    }
    let mut device: Device<EmulatorBackend> =
        Device::new(AudioBackend::new().unwrap_or_else(|| {
            error(
                clap::ErrorKind::Io,
                format_args!("Failed finding an audio output device"),
            )
        }));
    device.load_cartridge(cartridge);

    let size = winit::dpi::LogicalSize::new(500i32, 500i32);
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
        .unwrap_or_else(|err| {
            error(
                clap::ErrorKind::Io,
                format_args!("Failure while creating window ({})", err),
            )
        });

    event_loop.run(move |ev, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match ev {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                _ => (),
            },
            Event::MainEventsCleared => {
                if device.spc.backend.ahead.load(Ordering::Relaxed) < 20_000 {
                    device.run_cycle::<1>();
                    while !device.new_frame {
                        device.run_cycle::<1>();
                    }
                    window.request_redraw();
                }
            }
            Event::RedrawRequested(_) => {
                // TODO: render code
            }
            _ => (),
        }
    })
}
