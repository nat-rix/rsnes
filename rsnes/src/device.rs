//! The SNES/Famicom device

use crate::{
    backend::{AudioBackend, FrameBuffer},
    cartridge::Cartridge,
    controller::ControllerPorts,
    cpu::Cpu,
    dma::Dma,
    ppu::Ppu,
    registers::MathRegisters,
    smp::Smp,
    timing::Cycles,
};
use core::cell::Cell;
use save_state_macro::*;

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

impl save_state::InSaveState for Addr24 {
    fn serialize(&self, state: &mut save_state::SaveStateSerializer) {
        self.bank.serialize(state);
        self.addr.serialize(state);
    }

    fn deserialize(&mut self, state: &mut save_state::SaveStateDeserializer) {
        self.bank.deserialize(state);
        self.addr.deserialize(state);
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
        (self >> 8) as u8
    }
    fn from_open_bus(open_bus: u8) -> Self {
        open_bus as u16 | ((open_bus as u16) << 8)
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

#[derive(Debug, InSaveState)]
pub struct Device<B: AudioBackend, FB: FrameBuffer> {
    pub(crate) cpu: Cpu,
    pub smp: Smp<B>,
    pub ppu: Ppu<FB>,
    pub(crate) dma: Dma,
    pub controllers: ControllerPorts,
    pub(crate) cartridge: Option<Cartridge>,
    /// <https://wiki.superfamicom.org/open-bus>
    pub(crate) open_bus: u8,
    ram: [u8; RAM_SIZE],
    wram_addr: Cell<u32>,
    pub(crate) memory_cycles: Cycles,
    pub(crate) cpu_ahead_cycles: i32,
    pub(crate) new_scanline: bool,
    pub(crate) scanline_drawn: bool,
    pub new_frame: bool,
    pub(crate) do_hdma: bool,
    // multiplied by 4
    pub(crate) irq_time_h: u16,
    pub(crate) irq_time_v: u16,
    pub(crate) shall_irq: bool,
    pub(crate) shall_nmi: bool,
    pub(crate) nmi_vblank_bit: Cell<bool>,
    pub(crate) math_registers: MathRegisters,
    pub(crate) is_pal: bool,
}

impl<B: AudioBackend, FB: FrameBuffer> Device<B, FB> {
    pub fn new(audio_backend: B, frame_buffer: FB, is_pal: bool, is_threaded: bool) -> Self {
        Self {
            cpu: Cpu::new(),
            smp: Smp::new(audio_backend, is_pal, is_threaded),
            ppu: Ppu::new(frame_buffer, is_pal),
            dma: Dma::new(),
            controllers: ControllerPorts::new(),
            cartridge: None,
            open_bus: 0,
            ram: [0; RAM_SIZE],
            wram_addr: Cell::new(0),
            memory_cycles: 0,
            cpu_ahead_cycles: 186,
            new_scanline: true,
            new_frame: true,
            scanline_drawn: false,
            do_hdma: true,
            irq_time_h: 0x7fc,
            irq_time_v: 0x1ff,
            shall_irq: false,
            shall_nmi: false,
            nmi_vblank_bit: Cell::new(false),
            math_registers: MathRegisters::new(),
            is_pal,
        }
    }

    pub fn with_main_cpu<'a>(
        &'a mut self,
    ) -> crate::instr::DeviceAccess<'a, crate::instr::AccessTypeMain, B, FB> {
        crate::instr::create_device_access(self)
    }

    pub fn with_sa1_cpu<'a>(
        &'a mut self,
    ) -> crate::instr::DeviceAccess<'a, crate::enhancement::sa1::AccessTypeSa1, B, FB> {
        crate::instr::create_device_access(self)
    }

    pub fn load_cartridge(&mut self, mut cartridge: Cartridge) {
        cartridge.set_region(self.is_pal);
        self.cartridge = Some(cartridge);
        self.cpu = Cpu::new();
        self.reset_program_counter();
    }

    pub fn reset_program_counter(&mut self) {
        let addr = crate::cpu::RESET_VECTOR_ADDR;
        self.cpu.regs.pc = Addr24::new(0, self.read::<u16>(addr));
        if self.cartridge.as_ref().unwrap().has_sa1() {
            let vector = self.with_sa1_cpu().read::<u16>(addr);
            self.cartridge.as_mut().unwrap().sa1_mut().cpu_mut().regs.pc = Addr24::new(0, vector);
        }
    }

    /// Read a value from the mapped memory at the specified address.
    /// This method also updates open bus.
    pub fn read<D: Data>(&mut self, addr: Addr24) -> D {
        let value = self.read_data::<D>(addr);
        self.open_bus = value.to_open_bus();
        self.memory_cycles +=
            (self.get_memory_cycle(addr) - 6) * core::mem::size_of::<D::Arr>() as u32;
        value
    }

    /// Write a value to the mapped memory at the specified address.
    /// This method also updates open bus.
    pub fn write<D: Data>(&mut self, addr: Addr24, value: D) {
        self.open_bus = value.to_open_bus();
        self.write_data(addr, value);
        self.memory_cycles +=
            (self.get_memory_cycle(addr) - 6) * core::mem::size_of::<D::Arr>() as u32;
    }
}

impl<B: AudioBackend, FB: FrameBuffer> Device<B, FB> {
    pub fn read_bus_b<D: Data>(&mut self, addr: u8) -> D {
        let mut data = <D::Arr as Default>::default();

        for (i, d) in data.as_mut().iter_mut().enumerate() {
            let addr = addr.wrapping_add(i as u8);
            *d = match addr {
                0x34..=0x3f => {
                    let val = self.ppu.read_register(addr).unwrap_or(self.open_bus);
                    if addr < 0x3b || addr == 0x3e {
                        self.ppu.open_bus1 = val
                    } else {
                        self.ppu.open_bus2 = val
                    }
                    val
                }
                0x40..=0x7f => {
                    // APU Ports 2140h-2143h are mirrored to 2144h..217Fh
                    self.smp.read_output_port(addr)
                }
                0x80 => {
                    let res = self.ram[self.wram_addr.get() as usize];
                    self.increment_wram_addr();
                    res
                }
                0x00..=0x33 | 0x81..=0xff => self.open_bus,
            }
        }
        D::from_bytes(&data)
    }

    /// Read the mapped memory at the specified address
    ///
    /// # Note
    ///
    /// This method does not modify open bus.
    /// The master cycles aren't touched either.
    pub fn read_data<D: Data>(&mut self, addr: Addr24) -> D {
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
                    self.read_bus_b((addr.addr & 0xff) as u8)
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

    fn read_cartridge<D: Data>(&mut self, addr: Addr24) -> D {
        self.cartridge
            .as_mut()
            .unwrap()
            .read(addr)
            .unwrap_or_else(|| D::from_open_bus(self.open_bus))
    }

    fn increment_wram_addr(&self) {
        self.wram_addr.set(self.wram_addr.get().wrapping_add(1));
    }

    pub fn write_bus_b<D: Data>(&mut self, addr: u8, value: D) {
        for (i, d) in value.to_bytes().as_ref().iter().enumerate() {
            let addr = addr.wrapping_add(i as u8);
            match addr {
                0x00..=0x33 => self.ppu.write_register(addr, *d),
                0x40..=0x7f => self.smp.write_input_port(addr, *d),
                0x80 => {
                    self.ram[(self.wram_addr.get() & 0x1ffff) as usize] = *d;
                    self.increment_wram_addr();
                }
                0x81 => self
                    .wram_addr
                    .set((self.wram_addr.get() & 0xffff00) | u32::from(*d)),
                0x82 => self
                    .wram_addr
                    .set((self.wram_addr.get() & 0xff00ff) | (u32::from(*d) << 8)),
                0x83 => self
                    .wram_addr
                    .set((self.wram_addr.get() & 0xffff) | (u32::from(*d & 1) << 16)),
                0x34..=0x3f | 0x84..=0xff => (),
            }
        }
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
                    self.write_bus_b((addr.addr & 0xff) as u8, value)
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
