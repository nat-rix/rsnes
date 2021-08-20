extern crate snesulate;

use clap::{AppSettings, Clap};
use std::path::PathBuf;

#[derive(Clap, Clone)]
#[clap(
    version = clap::crate_version!(),
    setting = AppSettings::ColoredHelp
)]
struct Options {
    #[clap(parse(from_os_str))]
    input: PathBuf,
    #[clap(short, long)]
    verbose: bool,
}

fn main() {
    let options = Options::parse();

    let content = std::fs::read(&options.input).unwrap_or_else(|err| {
        clap::Error::with_description(
            format!(
                "Could not read file \"{}\" ({})\n",
                options.input.display(),
                err
            ),
            clap::ErrorKind::Io,
        )
        .exit()
    });
    let cartridge = snesulate::cartridge::Cartridge::from_bytes(&content).unwrap_or_else(|err| {
        clap::Error::with_description(
            format!(
                "Failiure while reading cartridge file \"{}\" ({})\n",
                options.input.display(),
                err
            ),
            clap::ErrorKind::InvalidValue,
        )
        .exit()
    });
    println!(
        "[info] Cartridge header information: {:#?}",
        cartridge.header()
    );
    let mut device = snesulate::device::Device::new();
    device.load_cartridge(cartridge);
}
