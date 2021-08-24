//! The SNES/Famicom device

use crate::{cartridge::Cartridge, cpu::Cpu, ppu::Ppu, spc700::Spc700};
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

pub trait Data: Sized + Default + Clone + Copy {
    type Arr: AsRef<[u8]> + AsMut<[u8]> + Default + std::fmt::Debug + Clone + Copy;
    fn to_bytes(self) -> Self::Arr;
    fn from_bytes(bytes: &Self::Arr) -> Self;

    fn parse(data: &[u8], index: usize) -> Self;
    fn write_to(self, data: &mut [u8], index: usize);

    fn to_open_bus(self) -> u8;
    fn from_open_bus(open_bus: u8) -> Self;
}

impl Data for u8 {
    type Arr = [u8; 1];
    fn to_bytes(self) -> [u8; 1] {
        [self]
    }
    fn from_bytes(bytes: &[u8; 1]) -> Self {
        bytes[0]
    }
    fn parse(data: &[u8], index: usize) -> Self {
        data[index]
    }
    fn write_to(self, data: &mut [u8], index: usize) {
        data[index] = self
    }
    fn to_open_bus(self) -> u8 {
        self
    }
    fn from_open_bus(open_bus: u8) -> Self {
        open_bus
    }
}

impl Data for u16 {
    type Arr = [u8; 2];
    fn to_bytes(self) -> [u8; 2] {
        self.to_le_bytes()
    }
    fn from_bytes(bytes: &[u8; 2]) -> Self {
        u16::from_le_bytes(*bytes)
    }
    fn parse(data: &[u8], index: usize) -> Self {
        u16::from_le_bytes(data[index..index + 2].try_into().unwrap())
    }
    fn write_to(self, data: &mut [u8], index: usize) {
        data[index..index + 2].copy_from_slice(&self.to_le_bytes())
    }
    fn to_open_bus(self) -> u8 {
        (self >> 8) as u8
    }
    fn from_open_bus(open_bus: u8) -> Self {
        open_bus as u16 | ((open_bus as u16) << 8)
    }
}

#[derive(Debug, Clone)]
pub struct Device {
    pub(crate) cpu: Cpu,
    pub(crate) spc: Spc700,
    pub(crate) ppu: Ppu,
    cartridge: Option<Cartridge>,
    /// <https://wiki.superfamicom.org/open-bus>
    open_bus: u8,
    ram: [u8; RAM_SIZE],
}

impl Device {
    pub fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            spc: Spc700::new(),
            ppu: Ppu::new(),
            cartridge: None,
            open_bus: 0,
            ram: [0; RAM_SIZE],
        }
    }

    pub fn load_cartridge(&mut self, cartridge: Cartridge) {
        self.cartridge = Some(cartridge);
        self.cpu = Cpu::new();
        self.reset_program_counter();
    }

    pub fn reset_program_counter(&mut self) {
        println!("resetting program counter from {:?}", self.cpu.regs.pc);
        self.cpu.regs.pc = Addr24::new(0, self.read::<u16>(Addr24::new(0, 0xfffc)));
        println!("to {:?}", self.cpu.regs.pc);
    }

    /// Fetch a value from the program counter memory region
    pub fn load<D: Data>(&mut self) -> D {
        let val = self.read::<D>(self.cpu.regs.pc);
        let len = core::mem::size_of::<D>() as u16;
        // yes, an overflow on addr does not carry the bank
        self.cpu.regs.pc.addr = self.cpu.regs.pc.addr.wrapping_add(len);
        val
    }

    /// Read a value from the mapped memory at the specified address.
    /// This method also updates open bus.
    pub fn read<D: Data>(&mut self, addr: Addr24) -> D {
        let value = self.read_data::<D>(addr);
        self.open_bus = value.to_open_bus();
        value
    }

    /// Write a value to the mapped memory at the specified address.
    /// This method also updates open bus.
    pub fn write<D: Data>(&mut self, addr: Addr24, value: D) {
        self.open_bus = value.to_open_bus();
        self.write_data(addr, value)
    }
}

impl Device {
    /// Read the mapped memory at the specified address
    ///
    /// # Note
    ///
    /// This method does not modify open bus
    pub fn read_data<D: Data>(&self, addr: Addr24) -> D {
        if (0x7e..=0x7f).contains(&addr.bank) {
            // address bus A + /WRAM
            D::parse(
                &self.ram,
                ((addr.bank as usize & 1) << 16) | addr.addr as usize,
            )
        } else if addr.bank & 0xc0 == 0 || addr.bank & 0xc0 == 0x80 {
            match addr.addr {
                0x0000..=0x1fff => {
                    // address bus A + /WRAM
                    D::parse(&self.ram, addr.addr as usize)
                }
                (0x2000..=0x20ff) | (0x2200..=0x3fff) | (0x4400..=0x7fff) => {
                    // address bus A
                    todo!()
                }
                0x2100..=0x21ff => {
                    // address bus B
                    match addr.addr {
                        0x2140..=0x2143 => D::parse(&self.spc.output, (addr.addr & 0b11) as usize),
                        _ => todo!("unimplemented address bus B read at 0x{:04x}", addr.addr),
                    }
                }
                0x4000..=0x43ff => {
                    // internal CPU registers
                    // see https://wiki.superfamicom.org/registers
                    let mut data = <D::Arr as Default>::default();
                    for (i, d) in data.as_mut().iter_mut().enumerate() {
                        *d = self
                            .cpu
                            .read_internal_register(addr.addr.wrapping_add(i as u16))
                            .unwrap_or(self.open_bus)
                    }
                    D::from_bytes(&data)
                }
                0x8000..=0xffff => {
                    // cartridge read on region $8000-$FFFF
                    self.cartridge
                        .as_ref()
                        .unwrap()
                        .read(addr)
                        .unwrap_or_else(|| D::from_open_bus(self.open_bus))
                }
            }
        } else {
            // cartridge read of bank $40-$7D or $C0-$FF
            todo!()
        }
    }

    /// Write the mapped memory at the specified address
    ///
    /// # Note
    ///
    /// This method does not modify open bus
    pub fn write_data<D: Data>(&mut self, addr: Addr24, value: D) {
        if (0x7e..=0x7f).contains(&addr.bank) {
            // address bus A + /WRAM
            value.write_to(
                &mut self.ram,
                ((addr.bank as usize & 1) << 16) | addr.addr as usize,
            )
        } else if addr.bank & 0xc0 == 0 || addr.bank & 0xc0 == 0x80 {
            match addr.addr {
                0x0000..=0x1fff => {
                    // address bus A + /WRAM
                    value.write_to(&mut self.ram, addr.addr as usize)
                }
                (0x2000..=0x20ff) | (0x2200..=0x3fff) | (0x4400..=0x7fff) => {
                    // address bus A
                    todo!()
                }
                0x2100..=0x21ff => {
                    // address bus B
                    match addr.addr {
                        0x2100..=0x2133 => {
                            for (i, d) in value.to_bytes().as_ref().iter().enumerate() {
                                self.ppu
                                    .write_register((addr.addr & 0xff) as u8 + i as u8, *d)
                            }
                        }
                        0x2140..=0x2143 => {
                            value.write_to(&mut self.spc.output, (addr.addr & 0b11) as usize)
                        }
                        _ => todo!("unimplemented address bus B read at 0x{:04x}", addr.addr),
                    }
                }
                0x4000..=0x43ff => {
                    // internal CPU registers
                    // see https://wiki.superfamicom.org/registers
                    for (i, d) in value.to_bytes().as_ref().iter().enumerate() {
                        self.cpu
                            .write_internal_register(addr.addr.wrapping_add(i as u16), *d)
                    }
                }
                0x8000..=0xffff => {
                    // cartridge read on region $8000-$FFFF
                    self.cartridge.as_mut().unwrap().write(addr, value)
                }
            }
        } else {
            // cartridge read of bank $40-$7D or $C0-$FF
            todo!()
        }
    }
}
