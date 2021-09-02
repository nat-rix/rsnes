use crate::cpu::Status;
use crate::device::{Addr24, Data, Device};
use crate::timing::Cycles;

#[rustfmt::skip]
static CYCLES: [Cycles; 256] = [
    /* ^0 ^1 ^2 ^3 ^4 ^5 ^6 ^7 | ^8 ^9 ^a ^b ^c ^d ^e ^f */
       0, 0, 7, 0, 0, 3, 0, 6,   3, 0, 2, 4, 0, 0, 0, 0,  // 0^
       2, 0, 0, 0, 0, 0, 0, 0,   2, 0, 2, 2, 0, 4, 0, 0,  // 1^
       6, 0, 8, 0, 0, 3, 0, 0,   4, 2, 0, 0, 0, 0, 0, 0,  // 2^
       2, 0, 0, 0, 0, 0, 0, 0,   2, 0, 0, 0, 0, 0, 0, 0,  // 3^
       0, 0, 0, 0, 0, 0, 0, 0,   3, 0, 0, 3, 3, 0, 0, 0,  // 4^
       0, 0, 0, 0, 1, 0, 0, 0,   2, 0, 3, 2, 4, 0, 0, 0,  // 5^
       6, 0, 0, 0, 3, 3, 0, 0,   0, 2, 0, 6, 0, 4, 0, 0,  // 6^
       0, 0, 0, 0, 2, 0, 0, 0,   2, 0, 4, 0, 0, 4, 0, 0,  // 7^
       3, 0, 0, 0, 3, 3, 3, 0,   2, 0, 2, 3, 4, 4, 4, 5,  // 8^
       2, 0, 0, 0, 0, 0, 0, 6,   2, 5, 2, 2, 4, 5, 5, 5,  // 9^
       2, 0, 2, 0, 3, 3, 3, 6,   2, 2, 2, 4, 0, 4, 4, 0,  // a^
       0, 0, 5, 0, 0, 0, 0, 6,   0, 3, 0, 2, 0, 3, 0, 0,  // b^
       2, 0, 3, 0, 0, 0, 5, 0,   2, 2, 2, 0, 0, 4, 0, 0,  // c^
       2, 0, 0, 0, 0, 0, 0, 0,   2, 0, 3, 0, 6, 0, 0, 0,  // d^
       2, 0, 3, 0, 0, 0, 5, 0,   2, 2, 0, 3, 0, 0, 0, 0,  // e^
       2, 0, 0, 0, 0, 0, 0, 0,   2, 0, 0, 2, 0, 4, 0, 0,  // f^
];

impl Device {
    fn load_indexed_v(&mut self, cycles: &mut Cycles, val: u16) -> Addr24 {
        let loaded_addr = self.load::<u16>();
        let addr = loaded_addr.wrapping_add(val);
        if self.cpu.is_idx8() || loaded_addr & 0xff00 != addr & 0xff00 {
            *cycles += 1
        }
        self.cpu.get_data_addr(addr)
    }

    /// Absolute Indexed, X
    pub fn load_indexed_x(&mut self, cycles: &mut Cycles) -> Addr24 {
        self.load_indexed_v(cycles, self.cpu.regs.x)
    }

    /// Absolute Indexed, Y
    pub fn load_indexed_y(&mut self, cycles: &mut Cycles) -> Addr24 {
        self.load_indexed_v(cycles, self.cpu.regs.y)
    }

    /// DP Indirect
    pub fn load_dp_indirect(&mut self, cycles: &mut Cycles) -> Addr24 {
        let addr = self.load::<u8>();
        if self.cpu.regs.dp & 0xff > 0 {
            *cycles += 1
        }
        let addr = self.read(Addr24::new(0, self.cpu.regs.dp.wrapping_add(addr.into())));
        self.cpu.get_data_addr(addr)
    }

    /// DP Indirect Long
    pub fn load_dp_indirect_long(&mut self, cycles: &mut Cycles) -> Addr24 {
        let addr = self.load::<u8>();
        if self.cpu.regs.dp & 0xff > 0 {
            *cycles += 1
        }
        self.read(Addr24::new(0, self.cpu.regs.dp.wrapping_add(addr.into())))
    }

    /// DP Indexed, X
    pub fn load_dp_indexed_x(&mut self, cycles: &mut Cycles) -> Addr24 {
        let addr = self.load::<u8>();
        if self.cpu.regs.dp & 0xff > 0 {
            *cycles += 1
        }
        Addr24::new(
            0,
            self.cpu
                .regs
                .dp
                .wrapping_add(addr.into())
                .wrapping_add(self.cpu.regs.x),
        )
    }

    /// Absolute Long Indexed, X
    pub fn load_long_indexed_x(&mut self) -> Addr24 {
        let Addr24 { mut bank, addr } = self.load::<Addr24>();
        let (addr, ov) = self.cpu.regs.x.overflowing_add(addr);
        if ov {
            bank = bank.wrapping_add(1)
        }
        Addr24::new(bank, addr)
    }

    /// Direct Page
    pub fn load_direct(&mut self, cycles: &mut Cycles) -> Addr24 {
        let val = self.load::<u8>();
        if self.cpu.regs.dp & 0xff > 0 {
            *cycles += 1
        }
        Addr24::new(0, self.cpu.regs.dp.wrapping_add(val.into()))
    }

    /// DP Indirect Long Indexed, Y
    pub fn load_indirect_long_indexed_y(&mut self, cycles: &mut Cycles) -> Addr24 {
        let addr = self.load::<u8>();
        if self.cpu.regs.dp & 0xff > 0 {
            *cycles += 1
        }
        let addr = self.read::<Addr24>(
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
                #[allow(unused_assignments)]
                if !self.cpu.regs.is_emulation {
                    cycles += 1
                }
                todo!("COP instruction")
            }
            0x05 => {
                // ORA - Or A with direct page
                let addr = self.load_direct(&mut cycles);
                if self.cpu.is_reg8() {
                    let val = self.read::<u8>(addr) | self.cpu.regs.a8();
                    self.cpu.regs.set_a8(val);
                    self.cpu.update_nz8(val);
                } else {
                    self.cpu.regs.a |= self.read::<u16>(addr);
                    self.cpu.update_nz16(self.cpu.regs.a);
                    cycles += 1
                }
            }
            0x07 => {
                // ORA - Or A with DP Indirect Long
                let addr = self.load_dp_indirect_long(&mut cycles);
                if self.cpu.is_reg8() {
                    let val = self.read::<u8>(addr) | self.cpu.regs.a8();
                    self.cpu.regs.set_a8(val);
                    self.cpu.update_nz8(val);
                } else {
                    self.cpu.regs.a |= self.read::<u16>(addr);
                    self.cpu.update_nz16(self.cpu.regs.a);
                    cycles += 1
                }
            }
            0x08 => {
                // PHP - Push Status Register
                self.push(self.cpu.regs.status.0)
            }
            0x0a => {
                // ASL - Arithmetic left shift on A
                if self.cpu.is_reg8() {
                    let val = self.cpu.regs.a8();
                    let newval = val << 1;
                    self.cpu.regs.set_a8(newval);
                    self.cpu.regs.status.set_if(Status::CARRY, val >= 0x80);
                    self.cpu.update_nz8(newval);
                } else {
                    let val = self.cpu.regs.a;
                    let newval = val << 1;
                    self.cpu.regs.a = newval;
                    self.cpu.regs.status.set_if(Status::CARRY, val >= 0x8000);
                    self.cpu.update_nz16(newval);
                }
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
            0x1a => {
                // INC/INA - Increment A
                if self.cpu.is_reg8() {
                    let a = self.cpu.regs.a8().wrapping_add(1);
                    self.cpu.regs.set_a8(a);
                    self.cpu.update_nz8(a)
                } else {
                    self.cpu.regs.a = self.cpu.regs.a.wrapping_add(1);
                    self.cpu.update_nz16(self.cpu.regs.a)
                }
            }
            0x1b => {
                // TCS - Transfer A to SP
                self.cpu.regs.sp = self.cpu.regs.a
            }
            0x1d => {
                // ORA - Or A with Absolute Indexed, X
                let addr = self.load_indexed_x(&mut cycles);
                if self.cpu.is_reg8() {
                    let val = self.read::<u8>(addr) | self.cpu.regs.a8();
                    self.cpu.regs.set_a8(val);
                    self.cpu.update_nz8(val);
                } else {
                    self.cpu.regs.a |= self.read::<u16>(addr);
                    self.cpu.update_nz16(self.cpu.regs.a);
                    cycles += 1
                }
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
            0x25 => {
                // AND - And A with direct page
                let addr = self.load_direct(&mut cycles);
                if self.cpu.is_reg8() {
                    let val = self.read::<u8>(addr) & self.cpu.regs.a8();
                    self.cpu.regs.set_a8(val);
                    self.cpu.update_nz8(val);
                } else {
                    self.cpu.regs.a &= self.read::<u16>(addr);
                    self.cpu.update_nz16(self.cpu.regs.a);
                    cycles += 1
                }
            }
            0x28 => {
                // PLP - Pull status
                self.cpu.regs.status = Status(self.pull::<u8>());
                self.cpu.update_status();
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
            0x30 => {
                // BMI - Branch if Negative Flag set
                self.branch_near(self.cpu.regs.status.has(Status::NEGATIVE), &mut cycles)
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
            0x4c => {
                // JMP - Jump absolute
                self.cpu.regs.pc.addr = self.load()
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
            0x58 => {
                // CLI - Clear IRQ_DISABLE
                // TODO: implement interrupts
                self.cpu.regs.status &= !Status::IRQ_DISABLE
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
            0x60 => {
                // RTS - Return from subroutine
                self.cpu.regs.pc.addr = 1u16.wrapping_add(self.pull());
            }
            0x64 => {
                // STZ - Store Zero to memory
                let addr = self.load_direct(&mut cycles);
                self.store_zero(addr, &mut cycles)
            }
            0x65 => {
                // ADC - DP Add with Carry
                let addr = self.load_direct(&mut cycles);
                if self.cpu.is_reg8() {
                    let op1 = self.read::<u8>(addr);
                    self.add_carry8(op1);
                } else {
                    let op1 = self.read::<u16>(addr);
                    self.add_carry16(op1);
                    cycles += 1;
                }
            }
            0x68 => {
                // PLA - Pull A
                if self.cpu.is_reg8() {
                    let a = self.pull();
                    self.cpu.regs.set_a8(a);
                    self.cpu.update_nz8(a);
                } else {
                    self.cpu.regs.a = self.pull();
                    self.cpu.update_nz16(self.cpu.regs.a);
                    cycles += 1
                }
            }
            0x69 => {
                // ADC -  immediate Add with Carry
                if self.cpu.is_reg8() {
                    let op1 = self.load::<u8>();
                    self.add_carry8(op1);
                } else {
                    let op1 = self.load::<u16>();
                    self.add_carry16(op1);
                    cycles += 1;
                }
            }
            0x6b => {
                // RTL - Return from subroutine long
                self.cpu.regs.pc = self.pull();
                self.cpu.regs.pc.addr = self.cpu.regs.pc.addr.wrapping_add(1);
            }
            0x6d => {
                // ADC - Add absolute with Carry
                let addr = self.load();
                let addr = self.cpu.get_data_addr(addr);
                if self.cpu.is_reg8() {
                    let op1 = self.read::<u8>(addr);
                    self.add_carry8(op1);
                } else {
                    let op1 = self.read::<u16>(addr);
                    self.add_carry16(op1);
                    cycles += 1;
                }
            }
            0x70 => {
                // BVS - Branch if Overflow is set
                self.branch_near(self.cpu.regs.status.has(Status::OVERFLOW), &mut cycles)
            }
            0x74 => {
                // STZ - Store Zero to DP X indexed memory
                let addr = self.load_dp_indexed_x(&mut cycles);
                self.store_zero(addr, &mut cycles)
            }
            0x78 => {
                // SEI - Set the Interrupt Disable flag
                self.cpu.regs.status |= Status::IRQ_DISABLE
            }
            0x7a => {
                // PLY - Pull Y
                if self.cpu.is_idx8() {
                    let y = self.pull();
                    self.cpu.regs.set_y8(y);
                    self.cpu.update_nz8(y);
                } else {
                    self.cpu.regs.y = self.pull();
                    self.cpu.update_nz16(self.cpu.regs.y);
                    cycles += 1
                }
            }
            0x7d => {
                // ADC - Add with Carry
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
            0x84 => {
                // STY - Store Y to direct page
                let addr = self.load_direct(&mut cycles);
                if self.cpu.is_idx8() {
                    self.write::<u8>(addr, self.cpu.regs.y8());
                } else {
                    self.write::<u16>(addr, self.cpu.regs.y);
                    cycles += 1;
                }
            }
            0x85 => {
                // STA - Store A to direct page
                let addr = self.load_direct(&mut cycles);
                if self.cpu.is_reg8() {
                    self.write::<u8>(addr, self.cpu.regs.a8());
                } else {
                    self.write::<u16>(addr, self.cpu.regs.a);
                    cycles += 1;
                }
            }
            0x86 => {
                // STX - Store X to direct page
                let addr = self.load_direct(&mut cycles);
                if self.cpu.is_idx8() {
                    self.write::<u8>(addr, self.cpu.regs.x8());
                } else {
                    self.write::<u16>(addr, self.cpu.regs.x);
                    cycles += 1;
                }
            }
            0x88 => {
                // DEY - Decrement Y
                if self.cpu.is_idx8() {
                    let y = self.cpu.regs.y8().wrapping_sub(1);
                    self.cpu.regs.set_y8(y);
                    self.cpu.update_nz8(y);
                } else {
                    self.cpu.regs.y = self.cpu.regs.y.wrapping_sub(1);
                    self.cpu.update_nz16(self.cpu.regs.y);
                }
            }
            0x8a => {
                // TXA - Transfer X to A
                if self.cpu.is_reg8() {
                    let val = self.cpu.regs.x8();
                    self.cpu.regs.set_a8(val);
                    self.cpu.update_nz8(val);
                } else {
                    let x = if self.cpu.is_idx8() {
                        self.cpu.regs.x8().into()
                    } else {
                        self.cpu.regs.x
                    };
                    self.cpu.regs.a = x;
                    self.cpu.update_nz16(x)
                }
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
            0x90 => {
                // BCC/BLT - Branch if Carry Clear
                self.branch_near(!self.cpu.regs.status.has(Status::CARRY), &mut cycles)
            }
            0x97 => {
                // STA - Store A to DP indirect long indexed, Y
                let addr = self.load_indirect_long_indexed_y(&mut cycles);
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
            0x99 => {
                // STA - Store A to absolute indexed Y
                let addr = self.load_indexed_y(&mut cycles);
                if self.cpu.is_reg8() {
                    self.write(addr, self.cpu.regs.a8());
                } else {
                    self.write(addr, self.cpu.regs.a);
                    cycles += 1
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
            0x9d => {
                // STA - Store A to absolute indexed X
                let addr = self.load_indexed_x(&mut cycles);
                if self.cpu.is_reg8() {
                    self.write(addr, self.cpu.regs.a8());
                } else {
                    self.write(addr, self.cpu.regs.a);
                    cycles += 1
                }
            }
            0x9e => {
                // STZ - absoulte X indexed
                let addr = self.load_indexed_x(&mut cycles);
                self.store_zero(addr, &mut cycles)
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
            0xa4 => {
                // LDY - Load direct page into Y
                let addr = self.load_direct(&mut cycles);
                if self.cpu.is_idx8() {
                    let y = self.read::<u8>(addr);
                    self.cpu.update_nz8(y);
                    self.cpu.regs.set_y8(y);
                } else {
                    let y = self.read::<u16>(addr);
                    self.cpu.update_nz16(y);
                    self.cpu.regs.y = y;
                    cycles += 1;
                }
            }
            0xa5 => {
                // LDA - Load direct page to A
                let addr = self.load_direct(&mut cycles);
                if self.cpu.is_reg8() {
                    let val = self.read(addr);
                    self.cpu.regs.set_a8(val);
                    self.cpu.update_nz8(val);
                } else {
                    let val = self.read(addr);
                    self.cpu.regs.a = val;
                    self.cpu.update_nz16(val);
                    cycles += 1;
                }
            }
            0xa6 => {
                // LDX - Load direct page into X
                let addr = self.load_direct(&mut cycles);
                if self.cpu.is_idx8() {
                    let x = self.read::<u8>(addr);
                    self.cpu.update_nz8(x);
                    self.cpu.regs.set_y8(x);
                } else {
                    let x = self.read::<u16>(addr);
                    self.cpu.update_nz16(x);
                    self.cpu.regs.x = x;
                    cycles += 1;
                }
            }
            0xa7 => {
                // LDA - Load direct page indirect long to A
                let addr = self.load_dp_indirect_long(&mut cycles);
                if self.cpu.is_reg8() {
                    let val = self.read(addr);
                    self.cpu.regs.set_a8(val);
                    self.cpu.update_nz8(val);
                } else {
                    let val = self.read(addr);
                    self.cpu.regs.a = val;
                    self.cpu.update_nz16(val);
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
            0xad => {
                // LDA - Load absolute to A
                let addr = self.load();
                let addr = self.cpu.get_data_addr(addr);
                if self.cpu.is_reg8() {
                    let val = self.read(addr);
                    self.cpu.regs.set_a8(val);
                    self.cpu.update_nz8(val);
                } else {
                    let val = self.read(addr);
                    self.cpu.regs.a = val;
                    self.cpu.update_nz16(val);
                    cycles += 1;
                }
            }
            0xae => {
                // LDX - Load absolute into X
                let addr = self.load::<u16>();
                let addr = self.cpu.get_data_addr(addr);
                if self.cpu.is_idx8() {
                    let x = self.read::<u8>(addr);
                    self.cpu.update_nz8(x);
                    self.cpu.regs.set_y8(x);
                } else {
                    let x = self.read::<u16>(addr);
                    self.cpu.update_nz16(x);
                    self.cpu.regs.x = x;
                    cycles += 1;
                }
            }
            0xb2 => {
                // LDA - Load DP indirect value to A
                let addr = self.load_dp_indirect(&mut cycles);
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
            0xb9 => {
                // LDA - Load absolute indexed Y value to A
                let addr = self.load_indexed_y(&mut cycles);
                if self.cpu.is_reg8() {
                    let val = self.read(addr);
                    self.cpu.regs.set_a8(val);
                    self.cpu.update_nz8(val);
                } else {
                    self.cpu.regs.a = self.read(addr);
                    self.cpu.update_nz16(self.cpu.regs.a);
                    cycles += 1
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
            0xbd => {
                // LDA - Load absolute indexed X value to A
                let addr = self.load_indexed_x(&mut cycles);
                if self.cpu.is_reg8() {
                    let val = self.read(addr);
                    self.cpu.regs.set_a8(val);
                    self.cpu.update_nz8(val);
                } else {
                    self.cpu.regs.a = self.read(addr);
                    self.cpu.update_nz16(self.cpu.regs.a);
                    cycles += 1
                }
            }
            0xc0 => {
                // CPY - Compare Y with immediate value
                // this will also work with decimal mode (TODO: check this fact)
                if self.cpu.is_idx8() {
                    let val = self.load::<u8>();
                    self.compare8(self.cpu.regs.y8(), val);
                } else {
                    let val = self.load::<u16>();
                    self.compare16(self.cpu.regs.y, val);
                    cycles += 1
                }
            }
            0xc2 => {
                // REP - Reset specified bits in the Status Register
                let mask = Status(!self.load::<u8>());
                // no `update_status` needed, because no bits got set
                self.cpu.regs.status &= mask
            }
            0xc6 => {
                // DEC - Decrement DP
                let addr = self.load_direct(&mut cycles);
                if self.cpu.is_reg8() {
                    let val = self.read::<u8>(addr).wrapping_sub(1);
                    self.write(addr, val);
                    self.cpu.update_nz8(val)
                } else {
                    let val = self.read::<u16>(addr).wrapping_sub(1);
                    self.write(addr, val);
                    self.cpu.update_nz16(val);
                    cycles += 2
                }
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
            0xc9 => {
                // CMP - Compare A with immediate value
                // this will also work with decimal mode (TODO: check this fact)
                if self.cpu.is_reg8() {
                    let val = self.load::<u8>();
                    self.compare8(self.cpu.regs.a8(), val);
                } else {
                    let val = self.load::<u16>();
                    self.compare16(self.cpu.regs.a, val);
                    cycles += 1
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
                    self.compare8(self.cpu.regs.a8(), val);
                } else {
                    let val = self.read::<u16>(addr);
                    self.compare16(self.cpu.regs.a, val);
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
            0xdc => {
                // JMP/JML - Jump absolute indirect long
                let addr = self.load();
                let addr = self.cpu.get_data_addr(addr);
                self.cpu.regs.pc = self.read::<Addr24>(addr);
            }
            0xe0 => {
                // CPX - Compare X with immediate value
                // this will also work with decimal mode (TODO: check this fact)
                if self.cpu.is_idx8() {
                    let val = self.load::<u8>();
                    self.compare8(self.cpu.regs.x8(), val);
                } else {
                    let val = self.load::<u16>();
                    self.compare16(self.cpu.regs.x, val);
                    cycles += 1
                }
            }
            0xe2 => {
                // SEP - Set specified bits in the Status Register
                let mask = Status(self.load::<u8>());
                self.cpu.regs.status |= mask;
                self.cpu.update_status();
            }
            0xe6 => {
                // INC - Increment direct page
                let addr = self.load_direct(&mut cycles);
                if self.cpu.is_reg8() {
                    let val = self.read::<u8>(addr).wrapping_add(1);
                    self.write::<u8>(addr, val);
                    self.cpu.update_nz8(val);
                } else {
                    let val = self.read::<u16>(addr).wrapping_add(1);
                    self.write::<u16>(addr, val);
                    self.cpu.update_nz16(val);
                    cycles += 2
                }
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
                if self.cpu.is_reg8() {
                    let op1 = self.load::<u8>();
                    self.sub_carry8(op1);
                } else {
                    let op1 = self.load::<u16>();
                    self.sub_carry16(op1);
                    cycles += 1;
                }
            }
            0xeb => {
                // XBA - Swap the A Register
                self.cpu.regs.a = self.cpu.regs.a.swap_bytes();
                self.cpu.update_nz8(self.cpu.regs.a8())
            }
            0xf0 => {
                // BEQ - Branch if ZERO is set
                self.branch_near(self.cpu.regs.status.has(Status::ZERO), &mut cycles)
            }
            0xf8 => {
                // SED - Set Decimal flag
                self.cpu.regs.status |= Status::DECIMAL
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
                let addr = self.load_indexed_x(&mut cycles);
                if self.cpu.is_reg8() {
                    let op1 = self.read::<u8>(addr);
                    self.sub_carry8(op1);
                } else {
                    let op1 = self.read::<u16>(addr);
                    self.sub_carry16(op1);
                    cycles += 1;
                }
            }
            opcode => todo!("not yet implemented CPU instruction 0x{:02x}", opcode),
        };
        cycles
    }

    fn generic_add_carry8<const GT1: u8, const GT2: u16>(
        &mut self,
        op1: u8,
        fu8: fn(u8, u8) -> u8,
        gt8: fn(&u8, &u8) -> bool,
        fu16: fn(u16, u16) -> u16,
        gt16: fn(&u16, &u16) -> bool,
    ) {
        let op2 = self.cpu.regs.a8();
        if self.cpu.regs.status.has(Status::DECIMAL) {
            let res = (op1 & 0xf)
                .wrapping_add(op2 & 0xf)
                .wrapping_add(self.cpu.regs.status.has(Status::CARRY) as _);
            let res = if gt8(&res, &GT1) { fu8(res, 6) } else { res };
            let carry = (res > 0xf) as u16;
            let res = u16::from(op1 & 0xf0)
                .wrapping_add((op2 & 0xf0).into())
                .wrapping_add(carry << 4)
                .wrapping_add((res & 0xf).into());
            self.cpu.regs.status.set_if(
                Status::OVERFLOW,
                !(u16::from(op1) ^ u16::from(op2)) & (u16::from(op2) ^ res) & 0x80 > 0,
            );
            let res = if gt16(&res, &GT2) {
                fu16(res, 0x60)
            } else {
                res
            };
            self.cpu.regs.status.set_if(Status::CARRY, res > 0xff);
            let res = (res & 0xff) as u8;
            self.cpu.update_nz8(res);
            self.cpu.regs.set_a8(res);
        } else {
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
    }

    pub fn add_carry8(&mut self, op1: u8) {
        self.generic_add_carry8::<9, 0x9f>(
            op1,
            u8::wrapping_add,
            u8::gt,
            u16::wrapping_add,
            u16::gt,
        )
    }

    pub fn sub_carry8(&mut self, op1: u8) {
        self.generic_add_carry8::<0xf, 0xff>(
            !op1,
            u8::wrapping_sub,
            u8::le,
            u16::wrapping_sub,
            u16::le,
        )
    }

    fn generic_add_carry16<const GT1: u16, const GT2: u16, const GT3: u16, const GT4: u32>(
        &mut self,
        op1: u16,
        f: fn(u16, u16) -> u16,
        gt: fn(&u16, &u16) -> bool,
        fu32: fn(u32, u32) -> u32,
        gt32: fn(&u32, &u32) -> bool,
    ) {
        let op2 = self.cpu.regs.a;
        if self.cpu.regs.status.has(Status::DECIMAL) {
            let res = (op1 & 0xf)
                .wrapping_add(op2 & 0xf)
                .wrapping_add(self.cpu.regs.status.has(Status::CARRY) as _);
            let res = if gt(&res, &GT1) { f(res, 6) } else { res };
            let carry = (res > 0xf) as u16;
            let res = (op1 & 0xf0)
                .wrapping_add(op2 & 0xf0)
                .wrapping_add(carry << 4)
                .wrapping_add(res & 0xf);
            let res = if gt(&res, &GT2) { f(res, 0x60) } else { res };
            let carry = (res > 0xff) as u16;
            let res = (op1 & 0xf00)
                .wrapping_add(op2 & 0xf00)
                .wrapping_add(carry << 8)
                .wrapping_add(res & 0xff);
            let res = if gt(&res, &GT3) { f(res, 0x600) } else { res };
            let carry = (res > 0xfff) as u32;
            let res = u32::from(op1 & 0xf000)
                .wrapping_add((op2 & 0xf000).into())
                .wrapping_add(carry << 12)
                .wrapping_add((res & 0xfff).into());
            self.cpu.regs.status.set_if(
                Status::OVERFLOW,
                !(u32::from(op1) ^ u32::from(op2)) & (u32::from(op2) ^ res) & 0x8000 > 0,
            );
            let res = if gt32(&res, &GT4) {
                fu32(res, 0x6000)
            } else {
                res
            };
            self.cpu.regs.status.set_if(Status::CARRY, res > 0xffff);
            let res = (res & 0xffff) as u16;
            self.cpu.update_nz16(res);
            self.cpu.regs.a = res
        } else {
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
    }

    pub fn add_carry16(&mut self, op1: u16) {
        self.generic_add_carry16::<9, 0x9f, 0x9ff, 0x9fff>(
            op1,
            u16::wrapping_add,
            u16::gt,
            u32::wrapping_add,
            u32::gt,
        )
    }

    pub fn sub_carry16(&mut self, op1: u16) {
        self.generic_add_carry16::<0xf, 0xff, 0xfff, 0xffff>(
            !op1,
            u16::wrapping_sub,
            u16::le,
            u32::wrapping_sub,
            u32::le,
        )
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

    pub fn compare8(&mut self, a: u8, b: u8) {
        let res = a as u16 + (!b) as u16 + 1;
        self.cpu.regs.status.set_if(Status::CARRY, res > 0xff);
        self.cpu.update_nz8((res & 0xff) as u8);
    }

    pub fn compare16(&mut self, a: u16, b: u16) {
        let res = a as u32 + (!b) as u32 + 1;
        self.cpu.regs.status.set_if(Status::CARRY, res > 0xffff);
        self.cpu.update_nz16((res & 0xffff) as u16);
    }

    pub fn dispatch_instruction(&mut self) -> Cycles {
        let pc = self.cpu.regs.pc;
        let op = self.load::<u8>();
        self.dispatch_instruction_with(pc, op)
    }
}
