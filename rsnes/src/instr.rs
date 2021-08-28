use crate::cpu::Status;
use crate::device::{Addr24, Data, Device, InverseU16};
use crate::timing::Cycles;

#[rustfmt::skip]
static CYCLES: [Cycles; 256] = [
    /* ^0 ^1 ^2 ^3 ^4 ^5 ^6 ^7 | ^8 ^9 ^a ^b ^c ^d ^e ^f */
       0, 0, 7, 0, 0, 0, 0, 0,   0, 0, 0, 4, 0, 0, 0, 0,  // 0^
       2, 0, 0, 0, 0, 0, 0, 0,   2, 0, 0, 2, 0, 0, 0, 0,  // 1^
       6, 0, 8, 0, 0, 0, 0, 0,   2, 2, 0, 0, 0, 0, 0, 0,  // 2^
       0, 0, 0, 0, 0, 0, 0, 0,   2, 0, 0, 0, 0, 0, 0, 0,  // 3^
       0, 0, 0, 0, 0, 0, 0, 0,   3, 0, 0, 3, 0, 0, 0, 0,  // 4^
       0, 0, 0, 0, 1, 0, 0, 0,   0, 0, 3, 2, 4, 0, 0, 0,  // 5^
       0, 0, 0, 0, 3, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 6^
       0, 0, 0, 0, 0, 0, 0, 0,   2, 0, 0, 0, 0, 4, 0, 0,  // 7^
       3, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 3, 4, 4, 4, 5,  // 8^
       0, 0, 0, 0, 0, 0, 0, 0,   2, 0, 2, 2, 4, 0, 0, 5,  // 9^
       2, 0, 2, 0, 0, 0, 0, 0,   2, 2, 2, 4, 0, 0, 0, 0,  // a^
       0, 0, 0, 0, 0, 0, 0, 6,   0, 0, 0, 2, 0, 0, 0, 0,  // b^
       0, 0, 3, 0, 0, 0, 0, 0,   2, 0, 2, 0, 0, 4, 0, 0,  // c^
       2, 0, 0, 0, 0, 0, 0, 0,   2, 0, 3, 0, 0, 0, 0, 0,  // d^
       0, 0, 3, 0, 0, 0, 0, 0,   2, 2, 0, 3, 0, 0, 0, 0,  // e^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 2, 0, 4, 0, 0,  // f^
];

impl Device {
    pub fn load_indexed_x(&mut self, cycles: &mut Cycles) -> Addr24 {
        let (addr, ov) = self.cpu.regs.x.overflowing_add(self.load::<InverseU16>().0);
        if ov || self.cpu.regs.x == 0 {
            // TODO: check this criteria (very much not sure)
            *cycles += 1
        }
        Addr24::new(self.cpu.regs.db, addr)
    }

    pub fn load_long_indexed_x(&mut self) -> Addr24 {
        let Addr24 { mut bank, addr } = self.load::<Addr24>();
        let (addr, ov) = self.cpu.regs.x.overflowing_add(addr);
        if ov {
            bank = bank.wrapping_add(1)
        }
        Addr24::new(bank, addr)
    }

    pub fn load_direct(&mut self, cycles: &mut Cycles) -> Addr24 {
        let val = self.load::<u8>();
        if self.cpu.regs.dp & 0xff > 0 {
            *cycles += 1
        }
        Addr24::new(0, self.cpu.regs.dp.wrapping_add(val.into()))
    }

    pub fn load_indirect_long_indexed_y(&mut self, cycles: &mut Cycles) -> Addr24 {
        let addr = self.load::<u8>();
        if self.cpu.regs.dp & 0xff > 0 {
            *cycles += 1
        }
        let mut addr = self.read::<Addr24>(
            self.cpu
                .get_data_addr(self.cpu.regs.dp.wrapping_add(addr.into())),
        );
        let (new_addr, ov) = addr.addr.overflowing_add(self.cpu.regs.y);
        if ov {
            Addr24::new(addr.bank.wrapping_add(1), new_addr)
        } else {
            Addr24::new(addr.bank, new_addr)
        }
    }

    pub fn dispatch_instruction_with(&mut self, start_addr: Addr24, op: u8) -> Cycles {
        println!("<CPU> executing '{:02x}' @ {}", op, start_addr);
        let mut cycles = CYCLES[op as usize];
        match op {
            0x02 => {
                // COP - Co-Processor Enable
                if !self.cpu.regs.is_emulation {
                    cycles += 1
                }
                todo!("COP instruction")
            }
            0x08 => {
                // PHP - Push Status Register
                self.push(self.cpu.regs.status.0)
            }
            0x0b => {
                // PHD - Push Direct Page
                self.push(self.cpu.regs.dp)
            }
            0x10 => {
                // BPL - Branch if Plus
                self.branch_near(!self.cpu.regs.status.has(Status::NEGATIVE), &mut cycles)
            }
            0x18 => {
                // CLC - Clear the Carry Flag
                self.cpu.regs.status &= !Status::CARRY;
            }
            0x1b => {
                // TCS - Transfer A to SP
                self.cpu.regs.sp = self.cpu.regs.a
            }
            0x20 => {
                // JSR - Jump to Subroutine
                self.push(start_addr.addr.wrapping_add(2));
                let new_addr = self.load::<u16>();
                self.cpu.regs.pc.addr = new_addr;
            }
            0x22 => {
                // JSR/JSL - Jump to Subroutine Long
                self.push(start_addr.bank);
                self.push(start_addr.addr.wrapping_add(3));
                let new_addr = self.load::<Addr24>();
                self.cpu.regs.pc = new_addr;
            }
            0x29 => {
                // AND - bitwise and A with immediate value
                if self.cpu.is_reg8() {
                    let value = self.cpu.regs.a8() & self.load::<u8>();
                    self.cpu.regs.set_a8(value);
                    self.cpu.update_nz8(value);
                } else {
                    self.cpu.regs.a &= self.load::<u16>();
                    self.cpu.update_nz16(self.cpu.regs.a);
                    cycles += 1
                }
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
            0x38 => {
                // SEC - Set Carry Flag
                self.cpu.regs.status |= Status::CARRY
            }
            0x48 => {
                // PHK - Push A
                if self.cpu.is_reg8() {
                    self.push(self.cpu.regs.a8())
                } else {
                    self.push(self.cpu.regs.a);
                    cycles += 1
                }
            }
            0x4b => {
                // PHK - Push PC Bank
                self.push(self.cpu.regs.pc.bank)
            }
            0x54 => {
                // MVN - Block Move Negative
                let [dst, src] = self.load::<u16>().to_bytes();
                if self.cpu.is_idx8() || self.cpu.regs.is_emulation {
                    while self.cpu.regs.a < 0xffff {
                        let val = self.read::<u8>(Addr24::new(src, self.cpu.regs.x & 0xff));
                        self.write::<u8>(Addr24::new(dst, self.cpu.regs.y & 0xff), val);
                        self.cpu.regs.set_x8(self.cpu.regs.x8().wrapping_add(1));
                        self.cpu.regs.set_y8(self.cpu.regs.y8().wrapping_add(1));
                        self.cpu.regs.a = self.cpu.regs.a.wrapping_sub(1);
                        cycles += 7
                    }
                } else {
                    while self.cpu.regs.a < 0xffff {
                        let val = self.read::<u8>(Addr24::new(src, self.cpu.regs.x));
                        self.write::<u8>(Addr24::new(dst, self.cpu.regs.y), val);
                        self.cpu.regs.x = self.cpu.regs.x.wrapping_add(1);
                        self.cpu.regs.y = self.cpu.regs.y.wrapping_add(1);
                        self.cpu.regs.a = self.cpu.regs.a.wrapping_sub(1);
                        cycles += 7
                    }
                }
            }
            0x5a => {
                // PHY - Push Y
                if self.cpu.is_idx8() {
                    self.push(self.cpu.regs.y8())
                } else {
                    self.push(self.cpu.regs.y);
                    cycles += 1
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
            }
            0x64 => {
                // STZ - Store Zero to memory
                let addr = self.load_direct(&mut cycles);
                self.store_zero(addr, &mut cycles)
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
            0x80 => {
                // BRA - Branch always
                self.branch_near(true, &mut cycles);
            }
            0x8b => {
                // PHB - Push Data Bank
                self.push(self.cpu.regs.db)
            }
            0x8c => {
                // STY - Store Y to absolute address
                let addr = self.load::<u16>();
                let addr = self.cpu.get_data_addr(addr);
                if self.cpu.is_idx8() {
                    self.write::<u8>(addr, self.cpu.regs.y8());
                } else {
                    self.write::<u16>(addr, self.cpu.regs.y);
                    cycles += 1;
                }
            }
            0x8d => {
                // STA - Store A to absolute address
                let addr = self.load::<u16>();
                let addr = self.cpu.get_data_addr(addr);
                if self.cpu.is_reg8() {
                    self.write::<u8>(addr, self.cpu.regs.a8());
                } else {
                    self.write::<u16>(addr, self.cpu.regs.a);
                    cycles += 1;
                }
            }
            0x8e => {
                // STX - Store X to absolute address
                let addr = self.load::<u16>();
                let addr = self.cpu.get_data_addr(addr);
                if self.cpu.is_idx8() {
                    self.write::<u8>(addr, self.cpu.regs.x8());
                } else {
                    self.write::<u16>(addr, self.cpu.regs.x);
                    cycles += 1;
                }
            }
            0x8f => {
                // STA - Store A to absolute long address
                let addr = self.load::<Addr24>();
                if self.cpu.is_reg8() {
                    self.write::<u8>(addr, self.cpu.regs.a8());
                } else {
                    self.write::<u16>(addr, self.cpu.regs.a);
                    cycles += 1;
                }
            }
            0x98 => {
                // TYA - Transfer Y to A
                if self.cpu.is_reg8() {
                    self.cpu.regs.set_a8(self.cpu.regs.y8());
                    self.cpu.update_nz8(self.cpu.regs.a8())
                } else {
                    self.cpu.regs.a = self.cpu.regs.y;
                    self.cpu.update_nz16(self.cpu.regs.a)
                }
            }
            0x9a => {
                // TXS - Transfer X to SP
                self.cpu.regs.sp = self.cpu.regs.x
            }
            0x9b => {
                // TXY - Transfer X to Y
                if self.cpu.is_idx8() {
                    let x = self.cpu.regs.x8();
                    self.cpu.regs.set_y8(x);
                    self.cpu.update_nz8(x);
                } else {
                    self.cpu.regs.y = self.cpu.regs.x;
                    self.cpu.update_nz16(self.cpu.regs.x);
                }
            }
            0x9c => {
                // STZ - absolute addressing
                let addr = self.load::<u16>();
                self.store_zero(self.cpu.get_data_addr(addr), &mut cycles)
            }
            0x9f => {
                // STA - Store absolute long indexed A to address
                let addr = self.load_long_indexed_x();
                if self.cpu.is_reg8() {
                    self.write::<u8>(addr, self.cpu.regs.a8());
                } else {
                    self.write::<u16>(addr, self.cpu.regs.a);
                    cycles += 1;
                }
            }
            0xa0 => {
                // LDY - Load immediate into Y
                if self.cpu.is_idx8() {
                    let y = self.load::<u8>();
                    self.cpu.update_nz8(y);
                    self.cpu.regs.set_y8(y);
                } else {
                    let y = self.load::<u16>();
                    self.cpu.update_nz16(y);
                    self.cpu.regs.y = y;
                    cycles += 1;
                }
            }
            0xa2 => {
                // LDX - Load immediate into X
                if self.cpu.is_idx8() {
                    let x = self.load::<u8>();
                    self.cpu.update_nz8(x);
                    self.cpu.regs.set_x8(x);
                } else {
                    let x = self.load::<u16>();
                    self.cpu.update_nz16(x);
                    self.cpu.regs.x = x;
                    cycles += 1;
                }
            }
            0xa8 => {
                // TAY - Transfer A to Y
                if self.cpu.is_idx8() {
                    let y = self.cpu.regs.a8();
                    self.cpu.regs.set_y8(y);
                    self.cpu.update_nz8(y);
                } else {
                    self.cpu.regs.y = if self.cpu.is_reg8() {
                        self.cpu.regs.a8().into()
                    } else {
                        self.cpu.regs.a
                    };
                    self.cpu.update_nz16(self.cpu.regs.y);
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
            0xaa => {
                // TAX - Transfer A to X
                if self.cpu.is_idx8() {
                    let x = self.cpu.regs.a8();
                    self.cpu.regs.set_x8(x);
                    self.cpu.update_nz8(x);
                } else {
                    self.cpu.regs.x = if self.cpu.is_reg8() {
                        self.cpu.regs.a8().into()
                    } else {
                        self.cpu.regs.a
                    };
                    self.cpu.update_nz16(self.cpu.regs.x);
                }
            }
            0xab => {
                // PLB - Pull Data Bank
                self.cpu.regs.db = self.pull();
                self.cpu.update_nz8(self.cpu.regs.db)
            }
            0xb7 => {
                // LDA - Load indirect long indexed Y value to A
                let addr = self.load_indirect_long_indexed_y(&mut cycles);
                if self.cpu.is_reg8() {
                    let val = self.read::<u8>(addr);
                    self.cpu.update_nz8(val);
                    self.cpu.regs.set_a8(val)
                } else {
                    let val = self.read::<u16>(addr);
                    self.cpu.update_nz16(val);
                    self.cpu.regs.a = val;
                    cycles += 1;
                }
            }
            0xbb => {
                // TYX - Transfer Y to X
                if self.cpu.is_idx8() {
                    let y = self.cpu.regs.y8();
                    self.cpu.regs.set_x8(y);
                    self.cpu.update_nz8(y);
                } else {
                    self.cpu.regs.x = self.cpu.regs.y;
                    self.cpu.update_nz16(self.cpu.regs.y);
                }
            }
            0xc2 => {
                // REP - Reset specified bits in the Status Register
                let mask = Status(!self.load::<u8>());
                self.cpu.regs.status &= mask
            }
            0xc8 => {
                // INY - Increment Y
                if self.cpu.is_idx8() {
                    let y = self.cpu.regs.y8().wrapping_add(1);
                    self.cpu.regs.set_y8(y);
                    self.cpu.update_nz8(y);
                } else {
                    self.cpu.regs.y = self.cpu.regs.y.wrapping_add(1);
                    self.cpu.update_nz16(self.cpu.regs.y);
                }
            }
            0xca => {
                // DEX - Decrement X
                if self.cpu.is_idx8() {
                    let x = self.cpu.regs.x8().wrapping_sub(1);
                    self.cpu.regs.set_x8(x);
                    self.cpu.update_nz8(x);
                } else {
                    self.cpu.regs.x = self.cpu.regs.x.wrapping_sub(1);
                    self.cpu.update_nz16(self.cpu.regs.x);
                }
            }
            0xcd => {
                // CMP - Compare A with absolute value
                // this will also work with decimal mode (TODO: check this fact)
                let addr = self.load::<u16>();
                let addr = self.cpu.get_data_addr(addr);
                if self.cpu.is_reg8() {
                    let val = self.read::<u8>(addr);
                    let res = self.cpu.regs.a8() as u16 + (!val) as u16 + 1;
                    self.cpu.regs.status.set_if(Status::CARRY, res > 0xff);
                    self.cpu.update_nz8((res & 0xff) as u8);
                } else {
                    let val = self.read::<u16>(addr);
                    let res = self.cpu.regs.a as u32 + (!val) as u32 + 1;
                    self.cpu.regs.status.set_if(Status::CARRY, res > 0xffff);
                    self.cpu.update_nz16((res & 0xffff) as u16);
                    cycles += 1
                }
            }
            0xd0 => {
                // BNE - Branch if Zero Flag Clear
                self.branch_near(!self.cpu.regs.status.has(Status::ZERO), &mut cycles)
            }
            0xd8 => {
                // CLD - Clear Decimal Flag
                self.cpu.regs.status &= !Status::DECIMAL
            }
            0xda => {
                // PHX - Push X
                if self.cpu.is_idx8() {
                    self.push(self.cpu.regs.x8())
                } else {
                    self.push(self.cpu.regs.x);
                    cycles += 1
                }
            }
            0xe2 => {
                // SEP - Set specified bits in the Status Register
                let mask = Status(self.load::<u8>());
                self.cpu.regs.status |= mask
            }
            0xe8 => {
                // INX - Increment X
                if self.cpu.is_idx8() {
                    let x = self.cpu.regs.x8().wrapping_add(1);
                    self.cpu.regs.set_x8(x);
                    self.cpu.update_nz8(x);
                } else {
                    self.cpu.regs.x = self.cpu.regs.x.wrapping_add(1);
                    self.cpu.update_nz16(self.cpu.regs.x);
                }
            }
            0xe9 => {
                // SBC - Subtract with carry
                assert!(!self.cpu.regs.status.has(Status::DECIMAL)); // TODO: implement decimal
                if self.cpu.is_reg8() {
                    let op1 = !self.load::<u8>();
                    self.add_carry8(op1);
                } else {
                    let op1 = !self.load::<u16>();
                    self.add_carry16(op1);
                    cycles += 1;
                }
            }
            0xeb => {
                // XBA - Swap the A Register
                self.cpu.regs.a = self.cpu.regs.a.swap_bytes();
                self.cpu.update_nz8(self.cpu.regs.a8())
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
            opcode => todo!("not yet implemented CPU instruction 0x{:02x}", opcode),
        };
        cycles
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

    pub fn branch_near(&mut self, cond: bool, cycles: &mut Cycles) {
        let rel = self.load::<u8>();
        if cond {
            *cycles += 1;
            let new = if rel & 0x80 > 0 {
                let rel = 128 - (rel & 0x7f);
                self.cpu.regs.pc.addr.wrapping_sub(rel.into())
            } else {
                self.cpu.regs.pc.addr.wrapping_add(rel.into())
            };
            let old = core::mem::replace(&mut self.cpu.regs.pc.addr, new);
            if self.cpu.regs.is_emulation && old & 0xff00 != new & 0xff00 {
                *cycles += 1
            }
        }
    }

    pub fn store_zero(&mut self, addr: Addr24, cycles: &mut Cycles) {
        if self.cpu.is_reg8() {
            self.write(addr, 0u8);
        } else {
            self.write(addr, 0u16);
            *cycles += 1;
        }
    }

    pub fn dispatch_instruction(&mut self) -> Cycles {
        let pc = self.cpu.regs.pc;
        let op = self.load::<u8>();
        self.dispatch_instruction_with(pc, op)
    }
}
