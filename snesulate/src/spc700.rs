//! SPC700 Sound Coprocessor handling types
//!
//! # Literature
//!
//! - <https://wiki.superfamicom.org/spc700-reference>
//! - <https://emudev.de/q00-snes/spc700-the-audio-processor/>

use crate::timing::Cycles;

pub const MEMORY_SIZE: usize = 64 * 1024;

static ROM: [u8; 64] = [
    0xCD, 0xEF, 0xBD, 0xE8, 0x00, 0xC6, 0x1D, 0xD0, 0xFC, 0x8F, 0xAA, 0xF4, 0x8F, 0xBB, 0xF5, 0x78,
    0xCC, 0xF4, 0xD0, 0xFB, 0x2F, 0x19, 0xEB, 0xF4, 0xD0, 0xFC, 0x7E, 0xF4, 0xD0, 0x0B, 0xE4, 0xF5,
    0xCB, 0xF4, 0xD7, 0x00, 0xFC, 0xD0, 0xF3, 0xAB, 0x01, 0x10, 0xEF, 0x7E, 0xF4, 0x10, 0xEB, 0xBA,
    0xF6, 0xDA, 0x00, 0xBA, 0xF4, 0xC4, 0xF4, 0xDD, 0x5D, 0xD0, 0xDB, 0x1F, 0x00, 0x00, 0xC0, 0xFF,
];

#[rustfmt::skip]
static CYCLES: [Cycles; 256] = [
    /* ^0 ^1 ^2 ^3 ^4 ^5 ^6 ^7 | ^8 ^9 ^a ^b ^c ^d ^e ^f */
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 0^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 1^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 2^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 3^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 4^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 2, 0, 0,  // 5^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 6^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 2, 0, 0,  // 7^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 2, 0, 0,  // 8^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 2, 0, 0,  // 9^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // a^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 2, 0, 0,  // b^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 2, 0, 0,  // c^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 2, 0, 0,  // d^
       0, 0, 0, 0, 0, 0, 0, 0,   2, 0, 0, 0, 0, 0, 0, 0,  // e^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 2, 0, 0,  // f^
];

const F1_RESET: u8 = 0xb0;

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
        const fn generate_power_up_memory() -> [u8; MEMORY_SIZE] {
            let mut mem: [u8; MEMORY_SIZE] =
                unsafe { core::mem::transmute([[[0x00u8; 32], [0xffu8; 32]]; 1024]) };
            mem[0xf1] = F1_RESET;
            mem
        }
        const POWER_UP_MEMORY: [u8; MEMORY_SIZE] = generate_power_up_memory();
        Self {
            mem: POWER_UP_MEMORY,
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
        self.mem[0xf1] = F1_RESET;
        self.input = [0; 4];
        self.output = [0; 4];
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.sp = 0;
        // actually self.read16(0xfffe), but this will
        // always result in 0xffc0, because mem[0xf1] = 0xb0
        self.pc = 0xffc0;
        self.status = 0;
    }

    pub const fn is_rom_mapped(&self) -> bool {
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

    pub fn read_small(&self, addr: u8) -> u8 {
        if self.status & flags::ZERO_PAGE > 0 {
            self.read(u16::from(addr) | 0x100)
        } else {
            self.read(addr.into())
        }
    }

    pub fn write_small(&mut self, addr: u8, val: u8) {
        if self.status & flags::ZERO_PAGE > 0 {
            self.write(u16::from(addr) | 0x100, val)
        } else {
            self.write(addr.into(), val)
        }
    }

    pub fn load(&mut self) -> u8 {
        let val = self.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        val
    }

    pub fn dispatch_instruction(&mut self) -> Cycles {
        let start_addr = self.pc;
        let op = self.load();
        println!("<SPC700> executing '{:02x}' @ ${:04x}", op, start_addr);
        let mut cycles = CYCLES[op as usize];
        match op {
            0x5d => {
                // MOV - X := A
                self.x = self.a;
                self.update_nz8(self.x)
            }
            0x7d => {
                // MOV - A := X
                self.a = self.x;
                self.update_nz8(self.a)
            }
            0x8d => {
                // MOV - Y := IMM
                let addr = self.load();
                self.y = self.read_small(addr);
                self.update_nz8(self.y);
            }
            0x9d => {
                // MOV - X := SP
                self.x = self.sp;
                self.update_nz8(self.x);
            }
            0xbd => {
                // MOV - SP := X
                self.sp = self.x
            }
            0xcd => {
                // MOV - X := IMM
                let addr = self.load();
                self.x = self.read_small(addr);
                self.update_nz8(self.x);
            }
            0xdd => {
                // MOV - A := Y
                self.a = self.y;
                self.update_nz8(self.a)
            }
            0xe8 => {
                // MOV - A := IMM
                let addr = self.load();
                self.a = self.read_small(addr);
                self.update_nz8(self.x);
            }
            0xfd => {
                // MOV - Y := A
                self.y = self.a;
                self.update_nz8(self.y)
            }
            _ => todo!("not yet implemented SPC700 instruction 0x{:02x}", op),
        }
        cycles
    }

    pub fn update_nz8(&mut self, val: u8) {
        if val > 0 {
            self.status = (self.status & !(flags::ZERO | flags::SIGN)) | (val & flags::SIGN);
        } else {
            self.status = (self.status & !flags::SIGN) | flags::ZERO
        }
    }
}
