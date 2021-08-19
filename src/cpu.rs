//! 65816/65C816 CPU handling types
//!
//! # Literature
//!
//! - the [super famicom wiki page](https://wiki.superfamicom.org/65816-reference)
//! - <https://apprize.best/programming/65816/>

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
    /// The direct page register
    pub dp: u16,
    /// The program counter
    pub pc: u16,
    /// The data bank register
    pub db: u8,
    /// The program bank register
    pub pb: u8,
    /// The processor status
    pub status: Status,
    /// 6502 emulation mode
    is_emulation: bool,
}

/// Processor status flags
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct Status(u8);

impl Status {
    /// Negative Flag
    pub const NEGATIVE: Self = Self(0b1000_0000);
    /// Overflow Flag
    pub const OVERFLOW: Self = Self(0b0100_0000);
    /// Memory/Accumulator size
    ///  - `0`: 16-bit
    ///  - `1`: 8-bit
    /// **native only**
    pub const ACCUMULATION: Self = Self(0b0010_0000);
    /// Index register size
    ///  - `0`: 16-bit
    ///  - `1`: 8-bit
    /// **native only**
    pub const INDEX_REGISTER_SIZE: Self = Self(0b0001_0000);
    /// Decimal Flag
    pub const DECIMAL: Self = Self(0b0000_1000);
    /// IRQ-Disable Flag
    ///  - `0`: Enabled
    ///  - `0`: Disabled
    pub const IRQ_DISABLE: Self = Self(0b0000_0100);
    /// Zero Flag
    pub const ZERO: Self = Self(0b0000_0010);
    /// Carry Flag
    pub const CARRY: Self = Self(0b0000_0001);
    /// Break Flag, **6502 emulation mode only**
    pub const BREAK: Self = Self(0b0001_0000);
}

/// Structure for emulating the 65816 Processor
#[derive(Debug, Clone)]
pub struct Cpu {
    regs: Regs,
}
