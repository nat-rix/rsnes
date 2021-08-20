//! The SNES/Famicom device

use crate::{cartridge::Cartridge, cpu::Cpu};
use core::convert::TryInto;

const RAM_SIZE: usize = 0x20000;

/// The 24-bit address type used
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Addr24 {
    pub bank: u8,
    pub addr: u16,
}

impl Addr24 {
    pub const fn new(bank: u8, addr: u16) -> Self {
        Self { bank, addr }
    }

    pub const fn is_lower_half(&self) -> bool {
        self.addr < 0x8000
    }
}

pub trait Access {
    type Output: std::fmt::Debug + Clone + Copy;
    fn access_slice(&self, slice: &[u8], index: usize) -> Self::Output;
    fn on_err(device: &Device) -> Self::Output;
}

pub struct ReadAccessU8;
pub struct ReadAccessU16;

impl Access for ReadAccessU8 {
    type Output = u8;
    fn access_slice(&self, slice: &[u8], index: usize) -> u8 {
        slice[index]
    }
    fn on_err(device: &Device) -> u8 {
        device.open_bus
    }
}

impl Access for ReadAccessU16 {
    type Output = u16;
    fn access_slice(&self, slice: &[u8], index: usize) -> u16 {
        u16::from_le_bytes(slice[index..index + 2].try_into().unwrap())
    }
    fn on_err(device: &Device) -> u16 {
        ((device.open_bus as u16) << 8) | (device.open_bus as u16)
    }
}

#[derive(Debug, Clone)]
pub struct Device {
    cpu: Cpu,
    cartridge: Option<Cartridge>,
    /// https://wiki.superfamicom.org/open-bus
    open_bus: u8,
    ram: [u8; RAM_SIZE],
}

impl Device {
    pub fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            cartridge: None,
            open_bus: 0,
            ram: [0; RAM_SIZE],
        }
    }

    pub fn load_cartridge(&mut self, cartridge: Cartridge) {
        self.cartridge = Some(cartridge);
        self.cpu = Cpu::new();
        self.reset_program_counter()
    }

    pub fn reset_program_counter(&mut self) {
        self.cpu.regs.pc = Addr24::new(0, self.read_u16(Addr24::new(0, 0xfffc)));
    }

    pub fn read_u8(&mut self, addr: Addr24) -> u8 {
        let val = self.access(ReadAccessU8, addr);
        self.open_bus = val;
        val
    }

    pub fn read_u16(&mut self, addr: Addr24) -> u16 {
        let val = self.access(ReadAccessU16, addr);
        self.open_bus = (val & 0xff) as u8;
        val
    }

    pub fn access<A: Access>(&self, access: A, addr: Addr24) -> A::Output {
        if (0x7e..=0x7f).contains(&addr.bank) {
            // address bus A + /WRAM
            access.access_slice(
                &self.ram,
                ((addr.bank as usize & 1) << 16) | addr.addr as usize,
            )
        } else if addr.bank & 0xc0 == 0 || addr.bank & 0xc0 == 0x80 {
            match addr.addr {
                0x0000..=0x1fff => {
                    // address bus A + /WRAM
                    access.access_slice(&self.ram, addr.addr as usize)
                }
                (0x2000..=0x20ff) | (0x2200..=0x3fff) | (0x4400..=0x7fff) => {
                    // address bus A
                    todo!()
                }
                0x2100..=0x21ff => {
                    // address bus B
                    todo!()
                }
                0x4000..=0x43ff => {
                    // internal CPU registers
                    // see https://wiki.superfamicom.org/registers#old-style-joypad-registers-80
                    todo!()
                }
                0x8000..=0xffff => {
                    // cartridge read on region $8000-$FFFF
                    self.cartridge
                        .as_ref()
                        .unwrap()
                        .access(access, addr)
                        .unwrap_or_else(|| A::on_err(&self))
                }
            }
        } else {
            // cartridge read of bank $40-$7D or $C0-$FF
            todo!()
        }
    }
}
