use crate::device::{Addr24, Data};

pub const CHANNEL_COUNT: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct Channel {
    a_bus: Addr24,
    size: u16,
}

impl Channel {
    pub fn new() -> Self {
        Self {
            a_bus: Addr24::new(0xff, 0xffff),
            size: 0xffff,
        }
    }

    /// Write 8-bit value from Channel transfer values
    pub fn read(&self, id: u8) -> Option<u8> {
        todo!()
    }

    /// Write 8-bit value to Channel transfer values
    pub fn write(&mut self, id: u8, value: u8) {
        match id {
            2 => self.a_bus.addr = (self.a_bus.addr & 0xff00) | value as u16,
            3 => self.a_bus.addr = (self.a_bus.addr & 0xff) | ((value as u16) << 8),
            4 => self.a_bus.bank = value,
            5 => self.size = (self.size & 0xff00) | value as u16,
            6 => self.size = (self.size & 0xff) | ((value as u16) << 8),
            _ => todo!("unknown dma write id {}", id),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Dma {
    channels: [Channel; CHANNEL_COUNT],
}

impl Dma {
    pub fn new() -> Self {
        Self {
            channels: [Channel::new(); CHANNEL_COUNT],
        }
    }

    /// Read 8-bit from channel transfer values
    pub fn read(&self, addr: u16) -> Option<u8> {
        let channel = ((addr & 0xf0) >> 4) & 0b111;
        self.channels[channel as usize].read((addr & 0xf) as u8)
    }

    /// Write 8-bit to Channel transfer values
    pub fn write(&mut self, addr: u16, value: u8) {
        let channel = ((addr & 0xf0) >> 4) & 0b111;
        self.channels[channel as usize].write((addr & 0xf) as u8, value)
    }
}
