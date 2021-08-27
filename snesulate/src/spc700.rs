//! SPC700 Sound Coprocessor handling types
//!
//! # Literature
//!
//! - <https://wiki.superfamicom.org/spc700-reference>
//! - <https://emudev.de/q00-snes/spc700-the-audio-processor/>

pub const MEMORY_SIZE: usize = 64 * 1024;

static ROM: [u8; 64] = [
    0xCD, 0xEF, 0xBD, 0xE8, 0x00, 0xC6, 0x1D, 0xD0, 0xFC, 0x8F, 0xAA, 0xF4, 0x8F, 0xBB, 0xF5, 0x78,
    0xCC, 0xF4, 0xD0, 0xFB, 0x2F, 0x19, 0xEB, 0xF4, 0xD0, 0xFC, 0x7E, 0xF4, 0xD0, 0x0B, 0xE4, 0xF5,
    0xCB, 0xF4, 0xD7, 0x00, 0xFC, 0xD0, 0xF3, 0xAB, 0x01, 0x10, 0xEF, 0x7E, 0xF4, 0x10, 0xEB, 0xBA,
    0xF6, 0xDA, 0x00, 0xBA, 0xF4, 0xC4, 0xF4, 0xDD, 0x5D, 0xD0, 0xDB, 0x1F, 0x00, 0x00, 0xC0, 0xFF,
];

/// Flags
pub mod flags {
    pub const CARRY: u8 = 0x01;
    pub const ZERO: u8 = 0x02;
    pub const INTERRUPT_ENABLE: u8 = 0x04;
    pub const HALF_CARRY: u8 = 0x08;
    pub const BREAK: u8 = 0x10;
    /// 0 means zero page is at 0x00xx,
    /// 1 means zero page is at 0x01xx
    pub const ZERO_PAGE: u8 = 0x20;
    pub const OVERFLOW: u8 = 0x40;
    pub const SIGN: u8 = 0x80;
}

#[derive(Debug, Clone)]
pub struct Spc700 {
    mem: [u8; MEMORY_SIZE],
    /// data, the main processor sends to us
    pub input: [u8; 4],
    /// data, we send to the main processor
    pub output: [u8; 4],

    a: u8,
    x: u8,
    y: u8,
    sp: u8,
    status: u8,
    pc: u16,
}

impl Spc700 {
    pub fn new() -> Self {
        Self {
            mem: [0; MEMORY_SIZE],
            input: [0; 4],
            output: [0; 4],
            a: 0,
            x: 0,
            y: 0,
            sp: 0,
            pc: 0xffc0,
            status: 0,
        }
    }

    pub fn reset(&mut self) {
        self.input = [0; 4];
        self.output = [0; 4];
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.sp = 0;
        self.pc = self.read16(0xfffe);
        self.status = 0;
    }

    pub fn is_rom_mapped(&self) -> bool {
        self.mem[0xf1] & 0x80 > 0
    }

    pub fn read16(&self, addr: u16) -> u16 {
        u16::from_le_bytes([self.read(addr), self.read(addr.wrapping_add(1))])
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xffc0..=0xffff if self.is_rom_mapped() => ROM[(addr & 0x3f) as usize],
            0xf4..=0xf7 => self.input[(addr - 0xf4) as usize],
            addr => self.mem[addr as usize],
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xf4..=0xf7 => self.output[(addr - 0xf4) as usize] = val,
            addr => self.mem[addr as usize] = val,
        }
    }
}
