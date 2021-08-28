//! The SNES/Famicom device

use crate::{cartridge::Cartridge, cpu::Cpu, dma::Dma, ppu::Ppu, spc700::Spc700};
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

impl std::fmt::Display for Addr24 {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:02x}:{:04x}", self.bank, self.addr)
    }
}

pub trait Data: std::fmt::Debug + Sized + Default + Clone + Copy {
    type Arr: AsRef<[u8]> + AsMut<[u8]> + Default + std::fmt::Debug + Clone + Copy;
    fn to_bytes(self) -> Self::Arr;
    fn from_bytes(bytes: &Self::Arr) -> Self;

    fn parse(data: &[u8], index: usize) -> Self;
    fn write_to(self, data: &mut [u8], index: usize);

    fn to_open_bus(self) -> u8;
    fn from_open_bus(open_bus: u8) -> Self;
}

#[repr(transparent)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InverseU16(pub u16);

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
        u16::from_le_bytes([data[index], data[(index + 1) % data.len()]])
    }
    fn write_to(self, data: &mut [u8], index: usize) {
        let [x, y] = self.to_bytes();
        data[index] = x;
        data[(index + 1) % data.len()] = y;
    }
    fn to_open_bus(self) -> u8 {
        (self & 0xff) as u8
    }
    fn from_open_bus(open_bus: u8) -> Self {
        open_bus as u16 | ((open_bus as u16) << 8)
    }
}

impl Data for InverseU16 {
    type Arr = [u8; 2];
    fn to_bytes(self) -> [u8; 2] {
        self.0.to_be_bytes()
    }
    fn from_bytes(bytes: &[u8; 2]) -> Self {
        Self(u16::from_be_bytes(*bytes))
    }
    fn parse(data: &[u8], index: usize) -> Self {
        Self(u16::from_be_bytes([
            data[index],
            data[(index + 1) % data.len()],
        ]))
    }
    fn write_to(self, data: &mut [u8], index: usize) {
        let [x, y] = self.to_bytes();
        data[index] = x;
        data[(index + 1) % data.len()] = y;
    }
    fn to_open_bus(self) -> u8 {
        (self.0 >> 8) as u8
    }
    fn from_open_bus(open_bus: u8) -> Self {
        Self(open_bus as u16 | ((open_bus as u16) << 8))
    }
}

impl Data for Addr24 {
    type Arr = [u8; 3];
    fn to_bytes(self) -> [u8; 3] {
        let bytes = self.addr.to_le_bytes();
        [bytes[0], bytes[1], self.bank]
    }
    fn from_bytes(bytes: &[u8; 3]) -> Self {
        Self::new(bytes[2], u16::from_le_bytes([bytes[0], bytes[1]]))
    }
    fn parse(data: &[u8], index: usize) -> Self {
        Self::from_bytes(&[
            data[index],
            data[(index + 1) % data.len()],
            data[(index + 2) % data.len()],
        ])
    }
    fn write_to(self, data: &mut [u8], index: usize) {
        let [x, y, z] = self.to_bytes();
        data[index] = x;
        data[(index + 1) % data.len()] = y;
        data[(index + 2) % data.len()] = z;
    }
    fn to_open_bus(self) -> u8 {
        self.bank
    }
    fn from_open_bus(open_bus: u8) -> Self {
        Self::new(open_bus, open_bus as u16 | ((open_bus as u16) << 8))
    }
}

#[derive(Debug, Clone)]
pub struct Device {
    pub(crate) cpu: Cpu,
    pub(crate) spc: Spc700,
    pub(crate) ppu: Ppu,
    pub(crate) dma: Dma,
    cartridge: Option<Cartridge>,
    /// <https://wiki.superfamicom.org/open-bus>
    open_bus: u8,
    ram: [u8; RAM_SIZE],
    wram_addr: Addr24,
    pub(crate) master_cycle: u64,
    pub(crate) memory_cycles: u32,
    pub(crate) apu_cycles: u64,
}

impl Device {
    pub fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            spc: Spc700::new(),
            ppu: Ppu::new(),
            dma: Dma::new(),
            cartridge: None,
            open_bus: 0,
            ram: [0; RAM_SIZE],
            wram_addr: Addr24::default(),
            master_cycle: 0,
            memory_cycles: 0,
            apu_cycles: 0,
        }
    }

    pub fn load_cartridge(&mut self, cartridge: Cartridge) {
        self.cartridge = Some(cartridge);
        self.cpu = Cpu::new();
        self.reset_program_counter();
    }

    pub fn reset_program_counter(&mut self) {
        self.cpu.regs.pc = Addr24::new(0, self.read::<u16>(Addr24::new(0, 0xfffc)))
    }

    /// Fetch a value from the program counter memory region
    pub fn load<D: Data>(&mut self) -> D {
        let val = self.read::<D>(self.cpu.regs.pc);
        let len = core::mem::size_of::<D::Arr>() as u16;
        // yes, an overflow on addr does not carry the bank
        self.cpu.regs.pc.addr = self.cpu.regs.pc.addr.wrapping_add(len);
        val
    }

    /// Read a value from the mapped memory at the specified address.
    /// This method also updates open bus.
    pub fn read<D: Data>(&mut self, addr: Addr24) -> D {
        let value = self.read_data::<D>(addr);
        self.open_bus = value.to_open_bus();
        self.memory_cycles += self.get_memory_cycle(addr) * core::mem::size_of::<D::Arr>() as u32;
        value
    }

    /// Write a value to the mapped memory at the specified address.
    /// This method also updates open bus.
    pub fn write<D: Data>(&mut self, addr: Addr24, value: D) {
        self.open_bus = value.to_open_bus();
        self.write_data(addr, value);
        self.memory_cycles += self.get_memory_cycle(addr) * core::mem::size_of::<D::Arr>() as u32;
    }

    /// Push data on the stack
    pub fn push<D: Data>(&mut self, val: D) {
        for d in val.to_bytes().as_ref().iter().rev() {
            self.write(Addr24::new(0, self.cpu.regs.sp), *d);
            self.cpu.regs.sp = self.cpu.regs.sp.wrapping_sub(1);
            if self.cpu.regs.is_emulation {
                self.cpu.regs.sp = (self.cpu.regs.sp & 0xff) | 256
            }
        }
    }

    /// Pull data from the stack
    pub fn pull<D: Data>(&mut self) -> D {
        let mut arr = D::Arr::default();
        for d in arr.as_mut() {
            self.cpu.regs.sp = self.cpu.regs.sp.wrapping_add(1);
            if self.cpu.regs.is_emulation {
                self.cpu.regs.sp = (self.cpu.regs.sp & 0xff) | 256
            }
            *d = self.read(Addr24::new(0, self.cpu.regs.sp));
        }
        D::from_bytes(&arr)
    }
}

impl Device {
    /// Read the mapped memory at the specified address
    ///
    /// # Note
    ///
    /// This method does not modify open bus.
    /// The master cycles aren't touched either.
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
                (0x2000..=0x20ff) | (0x2200..=0x3fff) | (0x4400..=0x5fff) => {
                    // address bus A
                    // TODO: should there always be a cartridge access done?
                    self.read_cartridge(addr)
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
                            .read_internal_register(addr.addr.wrapping_add(i as u16))
                            .unwrap_or(self.open_bus)
                    }
                    D::from_bytes(&data)
                }
                0x6000..=0xffff => {
                    // cartridge read on region ($30-$3f):$6000-$7fff or $xy:$8000-$FFFF
                    self.read_cartridge(addr)
                }
            }
        } else {
            // cartridge read of bank $40-$7D or $C0-$FF
            self.read_cartridge(addr)
        }
    }

    fn read_cartridge<D: Data>(&self, addr: Addr24) -> D {
        self.cartridge
            .as_ref()
            .unwrap()
            .read(addr)
            .unwrap_or_else(|| D::from_open_bus(self.open_bus))
    }

    /// Write the mapped memory at the specified address
    ///
    /// # Note
    ///
    /// This method does not modify open bus
    /// The master cycles aren't touched either.
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
                (0x2000..=0x20ff) | (0x2200..=0x3fff) | (0x4400..=0x5fff) => {
                    // address bus A
                    // TODO: should there always be a cartridge access done?
                    self.write_cartridge(addr, value)
                }
                0x2100..=0x21ff => {
                    // address bus B
                    for (i, d) in value.to_bytes().as_ref().iter().enumerate() {
                        let addr = (addr.addr.wrapping_add(i as u16) & 0xff) as u8;
                        match addr {
                            0x00..=0x33 => self.ppu.write_register(addr, *d),
                            0x40..=0x43 => self.spc.input[(addr & 0b11) as usize] = *d,
                            0x81 => {
                                self.wram_addr.addr = (self.wram_addr.addr & 0xff00) | *d as u16
                            }
                            0x82 => {
                                self.wram_addr.addr =
                                    (self.wram_addr.addr & 0xff) | ((*d as u16) << 8)
                            }
                            0x83 => self.wram_addr.bank = *d,
                            _ => todo!("unimplemented address bus B read at 0x21{:04x}", addr),
                        }
                    }
                }
                0x4000..=0x43ff => {
                    // internal CPU registers
                    // see https://wiki.superfamicom.org/registers
                    for (i, d) in value.to_bytes().as_ref().iter().enumerate() {
                        self.write_internal_register(addr.addr.wrapping_add(i as u16), *d)
                    }
                }
                0x6000..=0xffff => {
                    // cartridge read of bank $40-$7D or $C0-$FF
                    self.write_cartridge(addr, value)
                }
            }
        } else {
            // cartridge read of bank $40-$7D or $C0-$FF
            self.write_cartridge(addr, value)
        }
    }

    fn write_cartridge<D: Data>(&mut self, addr: Addr24, value: D) {
        self.cartridge.as_mut().unwrap().write(addr, value)
    }
}
