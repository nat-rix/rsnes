//! 65816/65C816 CPU handling types
//!
//! # Literature
//!
//! - the [super famicom wiki page](https://wiki.superfamicom.org/65816-reference)
//! - <https://apprize.best/programming/65816/>
//! - <https://www.westerndesigncenter.com/wdc/documentation/w65c816s.pdf>
//! - <https://wiki.superfamicom.org/uploads/assembly-programming-manual-for-w65c816.pdf>

use crate::device::Addr24;
use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};

/// Structure containing the processor registers
#[derive(Debug, Clone)]
pub struct Regs {
    /// The accumulator register
    pub a: u16,
    /// The first index register
    pub x: u16,
    /// The second index register
    pub y: u16,
    /// The stack pointer
    pub sp: u16,
    /// The direct page register (the direct page is limited to bank zero)
    pub dp: u16,
    /// The program counter with the program bank register.
    pub pc: Addr24,
    /// The data bank register
    pub db: u8,
    /// The processor status
    pub status: Status,
    /// 6502 emulation mode
    pub is_emulation: bool,
}

impl Regs {}

/// Processor status flags
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct Status(u8);

macro_rules! bitor { ($t:ident, $($vs:ident)|*) => { $t($(<$t>::$vs.0)|*) }; }

impl Status {
    /// Negative Flag
    pub const NEGATIVE: Self = Self(0b1000_0000);
    /// Overflow Flag
    pub const OVERFLOW: Self = Self(0b0100_0000);
    /// Memory/Accumulator size
    ///  - `0`: 16-bit
    ///  - `1`: 8-bit
    ///
    /// **native only**
    pub const ACCUMULATION: Self = Self(0b0010_0000);
    /// Index register size
    ///  - `0`: 16-bit
    ///  - `1`: 8-bit
    ///
    /// **native only**
    pub const INDEX_REGISTER_SIZE: Self = Self(0b0001_0000);
    /// Decimal Flag
    pub const DECIMAL: Self = Self(0b0000_1000);
    /// IRQ-Disable Flag
    ///  - `0`: Enabled
    ///  - `0`: Disabled
    pub const IRQ_DISABLE: Self = Self(0b0000_0100);
    /// Zero Flag
    ///
    /// # Note
    ///
    /// this is not actually zero, but indicates that
    /// an operation resulted in writing a zero
    pub const ZERO: Self = Self(0b0000_0010);
    /// Carry Flag
    pub const CARRY: Self = Self(0b0000_0001);
    /// Break Flag
    ///
    /// **6502 emulation mode only**
    pub const BREAK: Self = Self(0b0001_0000);

    /// The value that the status register gets reset to
    pub const RESET_DEFAULT: Self = bitor!(Self, ACCUMULATION | INDEX_REGISTER_SIZE | IRQ_DISABLE);

    pub const fn has(&self, flag: Self) -> bool {
        self.0 & flag.0 > 0
    }
}

impl BitAnd for Status {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl BitOr for Status {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for Status {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0
    }
}

impl BitAndAssign for Status {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0
    }
}

impl Not for Status {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}

/// Structure for emulating the 65816 Processor
#[derive(Debug, Clone)]
pub struct Cpu {
    pub regs: Regs,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            regs: Regs {
                a: 0,
                x: 0,
                y: 0,
                sp: 0x100,
                dp: 0,
                pc: Addr24::default(),
                db: 0,
                status: Status::RESET_DEFAULT,
                is_emulation: true,
            },
        }
    }

    pub const fn get_data_addr(&self, addr: u16) -> Addr24 {
        Addr24::new(self.regs.db, addr)
    }
}
