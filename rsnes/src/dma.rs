use crate::device::{Addr24, Device};

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
            3 => self.a_bus.addr = (self.a_bus.addr & 0xff) | (u16::from(value) << 8),
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
    channels: [Channel; 8],
    running: bool,
    dma_enabled: u8,
    hdma_enabled: u8,
    pub(crate) ahead_cycles: i32,
}

impl Dma {
    pub fn new() -> Self {
        Self {
            channels: [Channel::new(); 8],
            running: false,
            dma_enabled: 0,
            hdma_enabled: 0,
            ahead_cycles: 0,
        }
    }

    pub fn reset_hdma(&mut self) {
        // TODO
    }

    /// Read 8-bit from channel transfer values
    pub fn read(&self, addr: u16) -> Option<u8> {
        let channel = (addr >> 4) & 0b111;
        self.channels
            .get(channel as usize)
            .unwrap()
            .read((addr & 0xf) as u8)
    }

    /// Write 8-bit to Channel transfer values
    pub fn write(&mut self, addr: u16, value: u8) {
        let channel = (addr >> 4) & 0b111;
        self.channels
            .get_mut(channel as usize)
            .unwrap()
            .write((addr & 0xf) as u8, value)
    }

    pub const fn is_dma_running(&self) -> bool {
        self.running
    }

    pub const fn is_hdma_running(&self) -> bool {
        self.hdma_enabled > 0
    }

    pub fn enable_dma(&mut self, value: u8) {
        let activated = value & !self.dma_enabled;
        self.dma_enabled = value;
        self.running = self.dma_enabled > 0;
        if activated > 0 {
            self.ahead_cycles += 18 + activated.count_ones() as i32 * 8
        }
    }

    pub fn enable_hdma(&mut self, value: u8) {
        self.hdma_enabled = value;
    }

    pub fn get_first_dma_channel_id(&mut self) -> Option<usize> {
        if let id @ 0..=7 = self.dma_enabled.trailing_zeros() {
            Some(id as usize)
        } else {
            None
        }
    }

    pub fn do_hdma(&mut self) {
        todo!("do hdma stuff")
    }
}

impl Device {
    fn transfer_dma_byte(&mut self, channel_id: usize, b_bus_offset: u8) {
        let channel = self.dma.channels.get(channel_id).unwrap();
        let b_bus = channel.b_bus.wrapping_add(b_bus_offset);
        if channel.control & flags::PPU_TO_CPU > 0 {
            // PPU -> CPU
            let addr = channel.a_bus;
            let value = if (0x2180..=0x2183).contains(&addr.addr) && (0x80..=0x83).contains(&b_bus)
            {
                self.open_bus
            } else {
                self.read_bus_b::<u8>(b_bus)
            };
            match addr.addr {
                0x2100..=0x21ff | 0x4300..=0x437f | 0x420b | 0x420c => (),
                _ => self.write(addr, value),
            }
        } else {
            // CPU -> PPU
            let addr = channel.a_bus;
            let value = match (addr.bank, addr.addr) {
                (
                    0x00..=0x3f | 0x80..=0xbf,
                    0x2100..=0x21ff | 0x4300..=0x437f | 0x420b | 0x420c,
                ) => self.open_bus,
                _ => self.read::<u8>(addr),
            };
            self.write_bus_b(b_bus, value)
        }
    }

    pub fn do_dma(&mut self, channel_id: usize) {
        // TODO: this all may be optimized, because multiple reads on the same address
        // (FIXED mode) are not necessary in most cases. So check for this cases!
        // One thing I could imagine (that would be nicely optimizable):
        // Maybe FIXED mode writes always the same data even if two reads
        // would result in different data
        let channel = self.dma.channels.get(channel_id).unwrap();
        println!("doing dma at channel {}: {:#?}", channel_id, channel);
        let offsets: &[u8] = match channel.control & flags::MODE {
            0b000 => &[0],
            0b001 => &[0, 1],
            0b010 | 0b110 => &[0, 0],
            0b011 | 0b111 => &[0, 0, 1, 1],
            0b100 => &[0, 1, 2, 3],
            0b101 => &[0, 1, 0, 1],
            0b1000..=u8::MAX => unreachable!(),
        };
        for i in offsets {
            self.transfer_dma_byte(channel_id, *i);
            self.dma.ahead_cycles += 8
        }
        let channel = self.dma.channels.get_mut(channel_id).unwrap();
        if channel.control & flags::FIEXD == 0 {
            if channel.control & flags::DECREMENT > 0 {
                channel.a_bus.addr = channel.a_bus.addr.wrapping_sub(1)
            } else {
                channel.a_bus.addr = channel.a_bus.addr.wrapping_add(1)
            }
        }
        channel.size = channel.size.wrapping_sub(1);
        if channel.size == 0 {
            self.dma.dma_enabled &= !(1 << channel_id);
        }
    }

    pub fn do_dma_first_channel(&mut self) {
        if let Some(channel) = self.dma.get_first_dma_channel_id() {
            self.do_dma(channel)
        } else {
            self.dma.running = false
        }
    }
}
