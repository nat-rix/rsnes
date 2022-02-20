//! DSP-n cartridge coprocessor handling types
//!
//! # Literature
//!
//! - https://www.caitsith2.com/snes/dsp/
//! - https://datasheet.datasheetarchive.com/originals/scans/Scans-003/Scans-0079458.pdf
//! - SNES book 2 - Section 3

use crate::timing::Cycles;
use save_state::{InSaveState, SaveStateDeserializer, SaveStateSerializer};
use save_state_macro::InSaveState;

pub const ROM_SIZE: usize = 0x2000;

#[derive(Debug, Clone, Copy, InSaveState)]
pub struct Stack {
    stack: [u16; 4],
    size: u8,
}

impl Stack {
    pub const fn new() -> Self {
        Self {
            stack: [0; 4],
            size: 0,
        }
    }

    pub fn push(&mut self, val: u16) {
        // TODO: what happens on a stack overflow?
        self.stack[usize::from(self.size)] = val;
        self.size = (self.size + 1) & 3;
    }

    pub fn pop(&mut self) -> u16 {
        // TODO: what happens on a stack underflow?
        self.size = self.size.wrapping_sub(1) & 3;
        self.stack[usize::from(self.size)]
    }
}

#[allow(dead_code)]
pub mod status {
    /// Output pin P0
    pub const P0: u16 = 0x0001;

    /// Output pin P1
    pub const P1: u16 = 0x0002;

    /// Interrupt enable
    pub const EI: u16 = 0x0080;

    /// 8-bit mode for Serial Input
    pub const SIC: u16 = 0x0100;

    /// 8-bit mode for Serial Output
    pub const SOC: u16 = 0x0200;

    /// 8-bit mode for Parallel IO
    pub const DRC: u16 = 0x0400;

    /// DMA enable
    pub const DMA: u16 = 0x0800;

    /// even bytes transferred in 16-bit Parallel IO
    pub const DRS: u16 = 0x1000;

    /// General purpose flag
    pub const USF0: u16 = 0x2000;

    /// General purpose flag
    pub const USF1: u16 = 0x4000;

    /// Flag that tells if the last primary bus
    /// access was by Parallel IO
    pub const RQM: u16 = 0x8000;

    pub const WRITABLE: u16 = !(RQM | DRS);
}

pub mod flag {
    pub const OV0: u8 = 1;
    pub const OV1: u8 = 2;
    pub const Z: u8 = 4;
    pub const C: u8 = 8;
    pub const S0: u8 = 0x10;
    pub const S1: u8 = 0x20;
}

#[derive(Debug, Clone, InSaveState)]
pub struct Dsp {
    /// Status flags
    status: u16,
    /// 8-bit data ram pointer (called dp)
    ramptr: u8,
    /// 10-bit data rom pointer (called rp)
    romptr: u16,
    /// 11-bit program counter
    pc: u16,
    /// 16-bit signed multiplication inputs (called K, L)
    mult: [i16; 2],
    /// 16-bit signed ALU inputs (called A, B)
    acc: [u16; 2],
    /// 6,6-bit ALU flags for A and B
    flag: [u8; 2],
    /// 16-bit Temporary storage registers
    temp: [u16; 2],
    /// The 4-layer stack of 11-bit values
    stack: Stack,
    /// 16-bit or 8-bit parallel port
    port: u16,
    irom: [u32; 0x800],
    drom: [u16; 0x400],
    ram: [u16; 0x100],
    ver: DspVersion,

    timing_proportion: (Cycles, Cycles),
    master_cycles: Cycles,
}

impl Default for Dsp {
    fn default() -> Self {
        Self::new(DspVersion::Dsp1B)
    }
}

impl Dsp {
    pub fn new(ver: DspVersion) -> Self {
        let (ref irom, ref drom) = ver.rom();
        Self {
            status: 0,
            ramptr: 0,
            romptr: 0,
            pc: 0,
            mult: [0; 2],
            acc: [0; 2],
            flag: [0; 2],
            temp: [0; 2],
            stack: Stack::new(),
            port: 0,
            irom: *irom,
            drom: *drom,
            ram: [0; 0x100],
            ver,
            timing_proportion: (0, 0),
            master_cycles: 0,
        }
    }

    pub const fn version(&self) -> DspVersion {
        self.ver
    }

    pub fn set_timing_proportion(&mut self, prop: (Cycles, Cycles)) {
        self.timing_proportion = prop
    }

    pub fn tick(&mut self, n: Cycles) {
        self.master_cycles += n * self.timing_proportion.1
    }

    pub fn refresh(&mut self) {
        let cycles = self.master_cycles / self.timing_proportion.0;
        self.master_cycles %= self.timing_proportion.0;
        for _ in 0..cycles {
            self.dispatch()
        }
    }

    pub fn read_sr(&mut self) -> u8 {
        self.status.to_le_bytes()[1]
    }

    pub fn read_dr(&mut self) -> u8 {
        if self.status & status::DRC > 0 {
            // 8-bit parallel mode
            self.status &= !status::RQM;
            self.port.to_le_bytes()[0]
        } else {
            // 16-bit parallel mode
            let drs = self.status & status::DRS > 0;
            self.status &= !((self.status & status::DRS) << 3); // DRS = 1 => RQM = 0
            self.status ^= status::DRS;
            self.port.to_le_bytes()[drs as usize]
        }
    }

    pub fn write_dr(&mut self, val: u8) {
        if self.status & status::DRC > 0 {
            // 8-bit parallel mode
            self.status &= !status::RQM;
            self.port = u16::from_le_bytes([val, self.port.to_le_bytes()[1]]);
        } else {
            // 16-bit parallel mode
            let drs = self.status & status::DRS > 0;
            self.status &= !((self.status & status::DRS) << 3); // DRS = 1 => RQM = 0
            self.status ^= status::DRS;
            let mut bytes = self.port.to_le_bytes();
            bytes[drs as usize] = val;
            self.port = u16::from_le_bytes(bytes);
        }
    }

    pub fn get_mult_result(&self) -> u32 {
        ((self.mult[0] as i32 * self.mult[1] as i32) as u32) << 1
    }

    pub fn dispatch(&mut self) {
        let op = self.irom[usize::from(self.pc)];
        self.pc = self.pc.wrapping_add(1) & 0x7ff;
        self.run_opcode(op);
    }

    pub fn run_opcode(&mut self, op: u32) {
        if op & 0x80_00_00 == 0 {
            self.alu_instruction(op)
        } else if op & 0x40_00_00 == 0 {
            self.jp_instruction(op)
        } else {
            self.ld_instruction(op)
        }
    }

    fn alu_instruction(&mut self, op: u32) {
        // this is put on the primary bus
        let src = match (op >> 4) & 15 {
            0 => self.temp[1],
            1 => self.acc[0],
            2 => self.acc[1],
            3 => self.temp[0],
            4 => self.ramptr.into(),
            5 => self.romptr,
            6 => self.drom[usize::from(self.romptr)],
            7 => 0x8000 - (self.flag[0] & flag::S1 > 0) as u16,
            8 => {
                self.status |= status::RQM;
                self.port
            }
            9 => self.port,
            10 => self.status,
            11 | 12 => 0, // serial port is unconnected
            13 => self.mult[0] as u16,
            14 => self.mult[1] as u16,
            15 => self.ram[usize::from(self.ramptr)],
            _ => unreachable!(),
        };

        let alu_op = (op >> 16) & 15;
        if alu_op > 0 {
            // ALU op is not a NOP
            let a = (op >> 15) as usize & 1;
            let acc = self.acc[a];
            let p = match (op >> 20) & 3 {
                0 => self.ram[usize::from(self.ramptr)],
                1 => src,
                2 => (self.get_mult_result() >> 16) as u16,
                3 => (self.get_mult_result() & 0xffff) as u16,
                _ => unreachable!(),
            };
            let c = (self.flag[a ^ 1] & flag::C > 0) as u16;
            let mut carry = false;
            let mut overflow_check = false;
            macro_rules! set_carry {
                ($e:expr) => {{
                    let (r, ov) = $e;
                    #[allow(unused_assignments)]
                    {
                        overflow_check = true;
                    }
                    carry |= ov;
                    r
                }};
                (($e:expr) , c = acc & $c:literal) => {{
                    carry |= acc & $c > 0;
                    $e
                }};
            }
            self.acc[a] = match alu_op {
                1 => acc | p,
                2 => acc & p,
                3 => acc ^ p,
                4 => set_carry!(acc.overflowing_sub(p)),
                5 => set_carry!(acc.overflowing_add(p)),
                6 => set_carry!(set_carry!(acc.overflowing_sub(p)).overflowing_sub(c)),
                7 => set_carry!(set_carry!(acc.overflowing_add(p)).overflowing_add(c)),
                8 => set_carry!(acc.overflowing_sub(1)),
                9 => set_carry!(acc.overflowing_add(1)),
                10 => !acc,
                11 => set_carry!((((acc as i16) >> 1) as u16), c = acc & 1),
                12 => set_carry!(((acc << 1) | c), c = acc & 0x8000),
                13 => (acc << 2) | 3,
                14 => (acc << 4) | 15,
                15 => (acc >> 8) | (acc << 8),
                _ => unreachable!(),
            };
            self.flag[a] &= !flag::Z & !flag::S0 & !flag::C & !flag::OV0;
            if self.acc[a] == 0 {
                self.flag[a] |= flag::Z
            } else if self.acc[a] & 0x8000 > 0 {
                self.flag[a] |= flag::S0
            }
            if carry {
                self.flag[a] |= flag::C
            }
            if overflow_check {
                let operand = self.acc[a].wrapping_sub(acc);
                // an overflow occurs if both operands have the same
                // sign, but the result has the inverted sign
                if (operand ^ acc) & 0x8000 == 0 && (operand ^ self.acc[a]) & 0x8000 > 0 {
                    self.flag[a] |= flag::OV0;
                    self.flag[a] ^= flag::OV1;
                    self.flag[a] = (self.flag[a] & !flag::S1) | ((self.flag[a] << 1) & flag::S1);
                }
            } else if self.flag[a] & flag::OV1 == 0 {
                self.flag[a] = (self.flag[a] & !flag::S1) | ((self.flag[a] << 1) & flag::S1);
            } else {
                self.flag[a] &= !flag::OV1
            }
        }

        // TODO: FullSNES states that some combinations are prohibited
        //  - SRC = 13 / 14 and DST = 11 / 12
        //  - SRC and DST address the same register
        //  - PSELECT = 0 and DST = 15
        // investigate what to do in these cases

        self.store_to(op, src);

        match (op >> 13) & 3 {
            1 => self.ramptr = (self.ramptr & 0xf0) | (self.ramptr.wrapping_add(1) & 15),
            2 => self.ramptr = (self.ramptr & 0xf0) | (self.ramptr.wrapping_sub(1) & 15),
            3 => self.ramptr &= 0xf0,
            _ => (),
        }

        self.ramptr ^= ((op >> 5) & 0xf0) as u8;

        if op & 0x100 > 0 {
            self.romptr = self.romptr.wrapping_sub(1) & 0x3ff
        }

        if op & 0x400000 > 0 {
            // return from function
            self.pc = self.stack.pop();
        }
    }

    fn jp_instruction(&mut self, op: u32) {
        const FLAGS: [u8; 6] = [flag::C, flag::Z, flag::OV0, flag::OV1, flag::S0, flag::S1];
        let jump = match (op >> 13) & 0x1ff {
            0x140 => {
                self.stack.push(self.pc);
                true
            }
            0x100 => true,
            op @ 0x80..=0xae => {
                (self.flag[(op >> 2) as usize & 1] & FLAGS[(op >> 3) as usize & 7] == 0)
                    ^ (op & 2 > 0)
            }
            0xb0 => self.ramptr & 15 == 0,
            0xb1 => self.ramptr & 15 != 0,
            0xb2 => self.ramptr & 15 == 15,
            0xb3 => self.ramptr & 15 != 15,
            0xb4..=0xba => todo!("serial port jp opcode"),
            0xbc => self.status & status::RQM == 0,
            0xbe => self.status & status::RQM != 0,
            op => todo!("dsp jp opcode {:03x}", op),
        };
        if jump {
            self.pc = ((op >> 2) & 0x7ff) as u16;
        }
    }

    fn ld_instruction(&mut self, op: u32) {
        self.store_to(op, ((op >> 6) & 0xffff) as u16)
    }

    fn store_to(&mut self, dst: u32, val: u16) {
        match dst & 15 {
            1 => self.acc[0] = val,
            2 => self.acc[1] = val,
            3 => self.temp[0] = val,
            4 => self.ramptr = (val & 0xff) as u8,
            5 => self.romptr = val & 0x3ff,
            6 => {
                self.port = val;
                self.status |= status::RQM
            }
            7 => self.status = (self.status & !status::WRITABLE) | (val & status::WRITABLE),
            10 => self.mult[0] = val as _,
            11 => self.mult = [val as _, self.drom[usize::from(self.romptr)] as _],
            12 => self.mult = [self.ram[usize::from(self.ramptr | 0x40)] as _, val as _],
            13 => self.mult[1] = val as _,
            14 => self.temp[1] = val,
            15 => self.ram[usize::from(self.ramptr)] = val,
            _ => (),
        }
    }
}

const DSP1_ROM_FILE: [u8; ROM_SIZE] = *include_bytes!("roms/dsp1.rom");
const DSP1B_ROM_FILE: [u8; ROM_SIZE] = *include_bytes!("roms/dsp1b.rom");
const DSP2_ROM_FILE: [u8; ROM_SIZE] = *include_bytes!("roms/dsp2.rom");
const DSP3_ROM_FILE: [u8; ROM_SIZE] = *include_bytes!("roms/dsp3.rom");
const DSP4_ROM_FILE: [u8; ROM_SIZE] = *include_bytes!("roms/dsp4.rom");

pub type Rom = ([u32; 0x800], [u16; 0x400]);

static DSP1_ROM: Rom = DspVersion::split_roms(DSP1_ROM_FILE);
static DSP1B_ROM: Rom = DspVersion::split_roms(DSP1B_ROM_FILE);
static DSP2_ROM: Rom = DspVersion::split_roms(DSP2_ROM_FILE);
static DSP3_ROM: Rom = DspVersion::split_roms(DSP3_ROM_FILE);
static DSP4_ROM: Rom = DspVersion::split_roms(DSP4_ROM_FILE);

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum DspVersion {
    Dsp1 = 0,
    Dsp1B = 1,
    Dsp2 = 2,
    Dsp3 = 3,
    Dsp4 = 4,
}

impl DspVersion {
    pub fn rom(&self) -> &'static Rom {
        match self {
            Self::Dsp1 => &DSP1_ROM,
            Self::Dsp1B => &DSP1B_ROM,
            Self::Dsp2 => &DSP2_ROM,
            Self::Dsp3 => &DSP3_ROM,
            Self::Dsp4 => &DSP4_ROM,
        }
    }

    const fn split_roms(rom: [u8; ROM_SIZE]) -> Rom {
        let mut irom = [0; 0x800];
        let mut drom = [0; 0x400];
        let mut n = 0;
        let mut i = 0;
        while i < 0x800 {
            irom[i] = u32::from_le_bytes([rom[n], rom[n + 1], rom[n + 2], 0]);
            n += 3;
            i += 1;
        }
        i = 0;
        while i < 0x400 {
            drom[i] = u16::from_le_bytes([rom[n], rom[n + 1]]);
            n += 2;
            i += 1;
        }
        (irom, drom)
    }
}

impl InSaveState for DspVersion {
    fn serialize(&self, state: &mut SaveStateSerializer) {
        (*self as u8).serialize(state)
    }

    fn deserialize(&mut self, state: &mut SaveStateDeserializer) {
        let mut i: u8 = 0;
        i.deserialize(state);
        *self = match i {
            0 => Self::Dsp1B,
            1 => Self::Dsp1,
            2 => Self::Dsp2,
            3 => Self::Dsp3,
            4 => Self::Dsp4,
            _ => panic!("unknown enum discriminant {}", i),
        }
    }
}
