use crate::device::Addr24;

pub const CHANNEL_COUNT: usize = 8;

pub mod flags {
    pub const MODE: u8 = 0b111;
    pub const FIEXD: u8 = 0x08;
    pub const DECREMENT: u8 = 0x10;
    pub const INDIRECT: u8 = 0x40;
    pub const PPU_TO_CPU: u8 = 0x80;
}

#[derive(Debug, Clone, Copy)]
pub struct Channel {
    a_bus: Addr24,
    b_bus: u8,
    size: u16,
    indirect_bank: u8,
    control: u8,
    unknown_register: u8,
    table: u16,
    line_counter: u8,
}

impl Channel {
    pub fn new() -> Self {
        Self {
            a_bus: Addr24::new(0xff, 0xffff),
            b_bus: 0xff,
            size: 0xffff,
            control: 0xff,
            indirect_bank: 0xff,
            unknown_register: 0xff,
            table: 0xffff,
            line_counter: 0xff,
        }
    }

    /// Write 8-bit value from Channel transfer values
    pub fn read(&self, id: u8) -> Option<u8> {
        Some(match id {
            0 => self.control,
            1 => self.b_bus,
            2 => (self.a_bus.addr & 0xff) as u8,
            3 => (self.a_bus.addr >> 8) as u8,
            4 => self.a_bus.bank,
            5 => (self.size & 0xff) as u8,
            6 => (self.size >> 8) as u8,
            7 => self.indirect_bank,
            8 => (self.table & 0xff) as u8,
            9 => (self.table >> 8) as u8,
            10 => self.line_counter,
            11 | 15 => self.unknown_register,
            12..=14 => return None,
            _ => todo!("unknown dma read id {}", id),
        })
    }

    /// Write 8-bit value to Channel transfer values
    pub fn write(&mut self, id: u8, value: u8) {
        match id {
            0 => self.control = value,
            1 => self.b_bus = value,
            2 => self.a_bus.addr = (self.a_bus.addr & 0xff00) | value as u16,
            3 => self.a_bus.addr = (self.a_bus.addr & 0xff) | ((value as u16) << 8),
            4 => self.a_bus.bank = value,
            5 => self.size = (self.size & 0xff00) | value as u16,
            6 => self.size = (self.size & 0xff) | ((value as u16) << 8),
            7 => self.indirect_bank = value,
            8 => self.table = (self.table & 0xff00) | value as u16,
            9 => self.table = (self.table & 0xff) | ((value as u16) << 8),
            10 => self.line_counter = value,
            11 | 15 => self.unknown_register = value,
            12..=14 => (),
            _ => todo!("unknown dma write id {}", id),
        }
    }

    pub const fn indirect_address(&self) -> Addr24 {
        Addr24::new(self.indirect_bank, self.size)
    }
}

#[derive(Debug, Clone)]
pub struct Dma {
    channels: [Channel; CHANNEL_COUNT],
    dma_enabled: u8,
    hdma_enabled: u8,
}

impl Dma {
    pub fn new() -> Self {
        Self {
            channels: [Channel::new(); CHANNEL_COUNT],
            dma_enabled: 0,
            hdma_enabled: 0,
        }
    }

    /// Read 8-bit from channel transfer values
    pub fn read(&self, addr: u16) -> Option<u8> {
        let channel = (addr >> 4) & 0b111;
        self.channels[channel as usize].read((addr & 0xf) as u8)
    }

    /// Write 8-bit to Channel transfer values
    pub fn write(&mut self, addr: u16, value: u8) {
        let channel = (addr >> 4) & 0b111;
        self.channels[channel as usize].write((addr & 0xf) as u8, value)
    }

    pub const fn is_dma_running(&self) -> bool {
        self.dma_enabled > 0
    }

    pub const fn is_hdma_running(&self) -> bool {
        self.hdma_enabled > 0
    }

    pub fn enable_dma(&mut self, value: u8) {
        self.dma_enabled = value;
    }

    pub fn enable_hdma(&mut self, value: u8) {
        self.hdma_enabled = value;
    }
}
