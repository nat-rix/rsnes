use crate::cpu::Status;
use crate::device::{Addr24, Device, InverseU16};

#[rustfmt::skip]
static CYCLES: [u8; 256] = [
    /* ^0 ^1 ^2 ^3 ^4 ^5 ^6 ^7 | ^8 ^9 ^a ^b ^c ^d ^e ^f */
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 0^
       0, 0, 0, 0, 0, 0, 0, 0,   2, 0, 0, 2, 0, 0, 0, 0,  // 1^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 2^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 3^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 4^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 2, 0, 0, 0, 0,  // 5^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 6^
       0, 0, 0, 0, 0, 0, 0, 0,   2, 0, 0, 0, 0, 4, 0, 0,  // 7^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 4, 0, 5,  // 8^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 4, 0, 0, 0,  // 9^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 2, 0, 0, 0, 0, 0, 0,  // a^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // b^
       0, 0, 3, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // c^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // d^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // e^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // f^
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
        println!("exec '{:02x}'", op);
        let mut cycles = CYCLES[op as usize];
        match op {
            0x18 => {
                // CLC - Clear the Carry Flag
                self.cpu.regs.status &= !Status::CARRY;
            }
            0x1b => {
                // TCS - Transfer A to SP
                self.cpu.regs.sp = self.cpu.regs.a
            }
            0x5b => {
                // TCD - Transfer A to DP
                self.cpu.update_nz16(self.cpu.regs.a);
                self.cpu.regs.dp = self.cpu.regs.a;
            }
            0x78 => {
                // SEI - Set the Interrupt Disable flag
                self.cpu.regs.status |= Status::IRQ_DISABLE
            }
            0x7d => {
                // ADC - Add with Carry
                let addr = self.load_indexed_x(&mut cycles);
                if self.cpu.is_reg8() {
                    let op1 = self.read::<u8>(addr);
                    let op2 = self.cpu.regs.a8();
                    let (new, nc) = op1.overflowing_add(op2);
                    let (new, nc) = if self.cpu.regs.status.has(Status::CARRY) {
                        let (new, nc2) = new.overflowing_add(1);
                        (new, nc || nc2)
                    } else {
                        (new, nc)
                    };
                    self.cpu.regs.status.set_if(Status::CARRY, nc);
                    let op1v = op1 & 128;
                    let v = op1v == (op2 & 128) && op1v != (new & 128);
                    self.cpu.regs.status.set_if(Status::OVERFLOW, v);
                    self.cpu.update_nz8(new);
                    self.cpu.regs.set_a8(new);
                } else {
                    let op1 = self.read::<u16>(addr);
                    let op2 = self.cpu.regs.a;
                    let (new, nc) = op1.overflowing_add(op2);
                    let (new, nc) = if self.cpu.regs.status.has(Status::CARRY) {
                        let (new, nc2) = new.overflowing_add(1);
                        (new, nc || nc2)
                    } else {
                        (new, nc)
                    };
                    self.cpu.regs.status.set_if(Status::CARRY, nc);
                    let op1v = op1 & 0x8000;
                    let v = op1v == (op2 & 0x8000) && op1v != (new & 0x8000);
                    self.cpu.regs.status.set_if(Status::OVERFLOW, v);
                    self.cpu.update_nz16(new);
                    self.cpu.regs.a = new;
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
            0xfb => {
                // XCE - Swap Carry and Emulation Flags
                match (
                    self.cpu.regs.is_emulation,
                    self.cpu.regs.status.has(Status::CARRY),
                ) {
                    (true, false) => {
                        self.cpu.regs.is_emulation = false;
                        self.cpu.regs.status |= Status::CARRY
                    }
                    (false, true) => {
                        self.cpu.regs.is_emulation = true;
                        self.cpu.regs.status &= !Status::CARRY
                    }
                    _ => (),
                }
            }
            opcode => todo!("not yet implemented instruction 0x{:02x}", opcode),
        };
        println!("ran '{:02x}' with {} cycles", op, cycles);
    }

    pub fn dispatch_instruction(&mut self) {
        let op = self.load::<u8>();
        self.dispatch_instruction_with(op)
    }
}
