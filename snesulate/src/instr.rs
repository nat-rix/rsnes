use crate::cpu::Status;
use crate::device::{Addr24, Device, InverseU16};

#[rustfmt::skip]
static CYCLES: [u8; 256] = [
    /* ^0 ^1 ^2 ^3 ^4 ^5 ^6 ^7 | ^8 ^9 ^a ^b ^c ^d ^e ^f */
       0, 0, 7, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 0^
       0, 0, 0, 0, 0, 0, 0, 0,   2, 0, 0, 2, 0, 0, 0, 0,  // 1^
       0, 0, 0, 0, 0, 0, 0, 0,   2, 0, 0, 0, 0, 0, 0, 0,  // 2^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 3^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 4^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 2, 4, 0, 0, 0,  // 5^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 6^
       0, 0, 0, 0, 0, 0, 0, 0,   2, 0, 0, 0, 0, 4, 0, 0,  // 7^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 4, 0, 5,  // 8^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 4, 0, 0, 0,  // 9^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 2, 0, 0, 0, 0, 0, 0,  // a^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // b^
       0, 0, 3, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // c^
       0, 0, 0, 0, 0, 0, 0, 0,   2, 0, 0, 0, 0, 0, 0, 0,  // d^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // e^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 2, 0, 4, 0, 0,  // f^
];

impl Device {
    pub fn load_indexed_x(&mut self, cycles: &mut u8) -> Addr24 {
        let (addr, ov) = self.cpu.regs.x.overflowing_add(self.load::<InverseU16>().0);
        if ov || self.cpu.regs.x == 0 {
            // TODO: check this criteria (very much not sure)
            *cycles += 1
        }
        Addr24::new(self.cpu.regs.db, addr)
    }

    pub fn dispatch_instruction_with(&mut self, op: u8) {
        println!("exec '{:02x}' @ {}", op, self.cpu.regs.pc);
        let mut cycles = CYCLES[op as usize];
        match op {
            0x02 => {
                // COP - Co-Processor Enable
                if !self.cpu.regs.is_emulation {
                    cycles += 1
                }
                todo!("COP instruction")
            }
            0x18 => {
                // CLC - Clear the Carry Flag
                self.cpu.regs.status &= !Status::CARRY;
            }
            0x1b => {
                // TCS - Transfer A to SP
                self.cpu.regs.sp = self.cpu.regs.a
            }
            0x2a => {
                // ROL - Rotate A left
                if self.cpu.is_reg8() {
                    let val = self.cpu.regs.a8();
                    let res = self.cpu.regs.status.has(Status::CARRY) as u8 | val << 1;
                    self.cpu.regs.status.set_if(Status::CARRY, val & 0x80 > 0);
                    self.cpu.update_nz8(res);
                    self.cpu.regs.set_a8(res);
                } else {
                    let res = self.cpu.regs.status.has(Status::CARRY) as u16 | self.cpu.regs.a << 1;
                    self.cpu
                        .regs
                        .status
                        .set_if(Status::CARRY, self.cpu.regs.a & 0x80 > 0);
                    self.cpu.update_nz16(res);
                    self.cpu.regs.a = res;
                }
            }
            0x5b => {
                // TCD - Transfer A to DP
                self.cpu.update_nz16(self.cpu.regs.a);
                self.cpu.regs.dp = self.cpu.regs.a;
            }
            0x5c => {
                // JMP/JML - Jump absolute Long
                self.cpu.regs.pc = self.load::<Addr24>();
                println!("updating pc to: {}", self.cpu.regs.pc);
            }
            0x78 => {
                // SEI - Set the Interrupt Disable flag
                self.cpu.regs.status |= Status::IRQ_DISABLE
            }
            0x7d => {
                // ADC - Add with Carry
                assert!(!self.cpu.regs.status.has(Status::DECIMAL)); // TODO: implement decimal
                let addr = self.load_indexed_x(&mut cycles);
                if self.cpu.is_reg8() {
                    let op1 = self.read::<u8>(addr);
                    self.add_carry8(op1);
                } else {
                    let op1 = self.read::<u16>(addr);
                    self.add_carry16(op1);
                    cycles += 1;
                }
            }
            0x8f => {
                // STA - Store absolute long A to address
                let addr = self.load::<Addr24>();
                if self.cpu.is_reg8() {
                    self.write::<u8>(addr, self.cpu.regs.a8());
                } else {
                    self.write::<u16>(addr, self.cpu.regs.a);
                    cycles += 1;
                }
            }
            0x9c => {
                // STZ - absolute addressing
                if self.cpu.is_reg8() {
                    let addr = self.load::<u16>();
                    self.write(self.cpu.get_data_addr(addr), 0u8);
                } else {
                    let addr = self.load::<u16>();
                    self.write(self.cpu.get_data_addr(addr), 0u16);
                    cycles += 1;
                }
            }
            0xa9 => {
                // LDA - Load immediate value to A
                if self.cpu.is_reg8() {
                    let val = self.load::<u8>();
                    self.cpu.update_nz8(val);
                    self.cpu.regs.set_a8(val)
                } else {
                    let val = self.load::<u16>();
                    self.cpu.update_nz16(val);
                    self.cpu.regs.a = val;
                    cycles += 1;
                }
            }
            0x8d => {
                // STA - Store absolute A to address
                let addr = self.load::<u16>();
                let addr = self.cpu.get_data_addr(addr);
                if self.cpu.is_reg8() {
                    self.write::<u8>(addr, self.cpu.regs.a8());
                } else {
                    self.write::<u16>(addr, self.cpu.regs.a);
                    cycles += 1;
                }
            }
            0xc2 => {
                // REP - Reset specified bits in the Status Register
                let mask = Status(!self.load::<u8>());
                self.cpu.regs.status &= mask
            }
            0xd8 => {
                // CLD - Clear Decimal Flag
                self.cpu.regs.status &= !Status::DECIMAL
            }
            0xfb => {
                // XCE - Swap Carry and Emulation Flags
                self.cpu.regs.status.set_if(
                    Status::CARRY,
                    core::mem::replace(
                        &mut self.cpu.regs.is_emulation,
                        self.cpu.regs.status.has(Status::CARRY),
                    ),
                );
            }
            0xfd => {
                // SBC - Subtract with carry
                assert!(!self.cpu.regs.status.has(Status::DECIMAL)); // TODO: implement decimal
                let addr = self.load_indexed_x(&mut cycles);
                if self.cpu.is_reg8() {
                    let op1 = !self.read::<u8>(addr);
                    self.add_carry8(op1);
                } else {
                    let op1 = !self.read::<u16>(addr);
                    self.add_carry16(op1);
                    cycles += 1;
                }
            }
            opcode => todo!("not yet implemented instruction 0x{:02x}", opcode),
        };
        println!("ran '{:02x}' with {} cycles", op, cycles);
    }

    pub fn add_carry8(&mut self, op1: u8) {
        let op2 = self.cpu.regs.a8();
        let (new, nc) = op1.overflowing_add(op2);
        let (new, nc2) = new.overflowing_add(self.cpu.regs.status.has(Status::CARRY) as _);
        let nc = nc ^ nc2;
        self.cpu.regs.status.set_if(Status::CARRY, nc);
        let op1v = op1 & 128;
        let v = op1v == (op2 & 128) && op1v != (new & 128);
        self.cpu.regs.status.set_if(Status::OVERFLOW, v);
        self.cpu.update_nz8(new);
        self.cpu.regs.set_a8(new);
    }

    pub fn add_carry16(&mut self, op1: u16) {
        let op2 = self.cpu.regs.a;
        let (new, nc) = op1.overflowing_add(op2);
        let (new, nc2) = new.overflowing_add(self.cpu.regs.status.has(Status::CARRY) as _);
        let nc = nc ^ nc2;
        self.cpu.regs.status.set_if(Status::CARRY, nc);
        let op1v = op1 & 0x8000;
        let v = op1v == (op2 & 0x8000) && op1v != (new & 0x8000);
        self.cpu.regs.status.set_if(Status::OVERFLOW, v);
        self.cpu.update_nz16(new);
        self.cpu.regs.a = new;
    }

    pub fn dispatch_instruction(&mut self) {
        let op = self.load::<u8>();
        self.dispatch_instruction_with(op)
    }
}
