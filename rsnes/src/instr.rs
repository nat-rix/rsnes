use crate::cpu::{Cpu, Status};
use crate::device::{Addr24, Data, Device};
use crate::timing::Cycles;

// 0x80 BRA: the 2 instead of 3 cycles are on purpose.
//           `branch_near` will increment the cycle count
#[rustfmt::skip]
static CYCLES: [Cycles; 256] = [
    /* ^0 ^1 ^2 ^3 ^4 ^5 ^6 ^7 | ^8 ^9 ^a ^b ^c ^d ^e ^f */
       7, 6, 7, 4, 5, 3, 5, 6,   3, 2, 2, 4, 6, 4, 6, 5,  // 0^
       2, 5, 5, 7, 5, 4, 6, 6,   2, 4, 2, 2, 6, 4, 7, 5,  // 1^
       6, 6, 8, 4, 3, 3, 5, 6,   4, 2, 2, 5, 4, 4, 6, 5,  // 2^
       2, 5, 5, 7, 4, 4, 6, 6,   2, 4, 2, 2, 4, 4, 7, 5,  // 3^
       6, 6, 2, 4, 1, 3, 5, 6,   3, 2, 2, 3, 3, 4, 6, 5,  // 4^
       2, 5, 5, 7, 1, 4, 6, 6,   2, 4, 3, 2, 4, 4, 7, 5,  // 5^
       6, 6, 6, 4, 3, 3, 5, 6,   4, 2, 2, 6, 5, 4, 6, 5,  // 6^
       2, 5, 5, 7, 4, 4, 6, 6,   2, 4, 4, 2, 6, 4, 7, 5,  // 7^
       2, 6, 4, 4, 3, 3, 3, 6,   2, 2, 2, 3, 4, 4, 4, 5,  // 8^
       2, 6, 5, 7, 4, 4, 4, 6,   2, 5, 2, 2, 4, 5, 5, 5,  // 9^
       2, 6, 2, 4, 3, 3, 3, 6,   2, 2, 2, 4, 4, 4, 4, 5,  // a^
       2, 5, 5, 7, 4, 4, 4, 6,   2, 4, 2, 2, 4, 4, 4, 5,  // b^
       2, 6, 3, 4, 3, 3, 5, 6,   2, 2, 2, 3, 4, 4, 6, 5,  // c^
       2, 5, 5, 7, 6, 4, 6, 6,   2, 4, 3, 3, 6, 4, 7, 5,  // d^
       2, 6, 3, 4, 3, 3, 5, 6,   2, 2, 2, 3, 4, 4, 6, 5,  // e^
       2, 5, 5, 7, 5, 4, 6, 6,   2, 4, 4, 2, 8, 4, 7, 5,  // f^
];

macro_rules! compare_memory {
    (CMP: $($t:tt)*) => {compare_memory!([a, a8, is_reg8]: $($t)*)};
    (CPX: $($t:tt)*) => {compare_memory!([x, x8, is_idx8]: $($t)*)};
    (CPY: $($t:tt)*) => {compare_memory!([y, y8, is_idx8]: $($t)*)};
    ([$r:ident, $r8:ident, $is8:ident]: $self:ident, $addr:expr, $cycles:expr) => {{
        // this will also work with decimal mode (TODO: check this fact)
        if $self.cpu().$is8() {
            let val = $self.read::<u8>($addr);
            $self.compare8($self.cpu().regs.$r8() as u8, val);
        } else {
            let val = $self.read::<u16>($addr);
            $self.compare16($self.cpu().regs.$r, val);
            *$cycles += 1
        }
    }};
}

pub trait AccessType<B: crate::backend::AudioBackend, FB: crate::backend::FrameBuffer> {
    fn read<D: Data>(device: &mut Device<B, FB>, addr: Addr24) -> D;
    fn write<D: Data>(device: &mut Device<B, FB>, addr: Addr24, val: D);
}

pub struct AccessTypeMain;

impl<B: crate::backend::AudioBackend, FB: crate::backend::FrameBuffer> AccessType<B, FB>
    for AccessTypeMain
{
    fn read<D: Data>(device: &mut Device<B, FB>, addr: Addr24) -> D {
        device.read::<D>(addr)
    }

    fn write<D: Data>(device: &mut Device<B, FB>, addr: Addr24, val: D) {
        device.write::<D>(addr, val)
    }
}

pub(crate) fn create_device_access<
    'a,
    T: AccessType<B, FB>,
    B: crate::backend::AudioBackend,
    FB: crate::backend::FrameBuffer,
>(
    device: &'a mut Device<B, FB>,
) -> DeviceAccess<'a, T, B, FB> {
    DeviceAccess(device, core::marker::PhantomData)
}

pub struct DeviceAccess<
    'a,
    T: AccessType<B, FB>,
    B: crate::backend::AudioBackend,
    FB: crate::backend::FrameBuffer,
>(&'a mut Device<B, FB>, core::marker::PhantomData<T>);

impl<
        'a,
        T: AccessType<B, FB>,
        B: crate::backend::AudioBackend,
        FB: crate::backend::FrameBuffer,
    > DeviceAccess<'a, T, B, FB>
{
    pub fn cpu(&self) -> &Cpu {
        &self.0.cpu
    }

    pub fn cpu_mut(&mut self) -> &mut Cpu {
        &mut self.0.cpu
    }

    pub fn read<D: Data>(&mut self, addr: Addr24) -> D {
        T::read(self.0, addr)
    }

    pub fn write<D: Data>(&mut self, addr: Addr24, val: D) {
        T::write(self.0, addr, val)
    }

    /// Fetch a value from the program counter memory region
    pub fn load<D: Data>(&mut self) -> D {
        let val = self.read::<D>(self.cpu().regs.pc);
        let len = core::mem::size_of::<D::Arr>() as u16;
        // yes, an overflow on addr does not carry the bank
        self.cpu_mut().regs.pc.addr = self.cpu().regs.pc.addr.wrapping_add(len);
        val
    }

    /// Push data on the stack
    pub fn push<D: Data>(&mut self, val: D) {
        for d in val.to_bytes().as_ref().iter().rev() {
            self.write(Addr24::new(0, self.cpu().regs.sp), *d);
            self.cpu_mut().regs.sp = self.cpu().regs.sp.wrapping_sub(1);
            if self.cpu().regs.is_emulation {
                self.cpu_mut().regs.sp = (self.cpu().regs.sp & 0xff) | 256
            }
        }
    }

    /// Pull data from the stack
    pub fn pull<D: Data>(&mut self) -> D {
        let mut arr = D::Arr::default();
        for d in arr.as_mut() {
            self.cpu_mut().regs.sp = self.cpu().regs.sp.wrapping_add(1);
            if self.cpu().regs.is_emulation {
                self.cpu_mut().regs.sp = (self.cpu().regs.sp & 0xff) | 256
            }
            *d = self.read(Addr24::new(0, self.cpu().regs.sp));
        }
        D::from_bytes(&arr)
    }
}

impl<
        'a,
        T: AccessType<B, FB>,
        B: crate::backend::AudioBackend,
        FB: crate::backend::FrameBuffer,
    > DeviceAccess<'a, T, B, FB>
{
    fn load_indexed_v<const BC: bool>(&mut self, cycles: &mut Cycles, val: u16) -> Addr24 {
        let loaded_addr = self.load::<u16>();
        let addr = loaded_addr.wrapping_add(val);
        if BC && (!self.cpu().is_idx8() || loaded_addr & 0xff00 != addr & 0xff00) {
            *cycles += 1
        }
        self.cpu().get_data_addr(addr)
    }

    /// Absolute Indexed, X
    pub fn load_indexed_x<const BC: bool>(&mut self, cycles: &mut Cycles) -> Addr24 {
        self.load_indexed_v::<BC>(
            cycles,
            if self.cpu().is_idx8() {
                self.cpu().regs.x & 0xff
            } else {
                self.cpu().regs.x
            },
        )
    }

    /// Absolute Indexed, Y
    pub fn load_indexed_y<const BC: bool>(&mut self, cycles: &mut Cycles) -> Addr24 {
        self.load_indexed_v::<BC>(
            cycles,
            if self.cpu().is_idx8() {
                self.cpu().regs.y & 0xff
            } else {
                self.cpu().regs.y
            },
        )
    }

    /// DP Indirect
    pub fn load_dp_indirect(&mut self, cycles: &mut Cycles) -> Addr24 {
        let addr = self.load::<u8>();
        if self.cpu().regs.dp & 0xff > 0 {
            *cycles += 1
        }
        let addr = self.read(Addr24::new(0, self.cpu().regs.dp.wrapping_add(addr.into())));
        self.cpu().get_data_addr(addr)
    }

    /// DP Indirect Long
    pub fn load_dp_indirect_long(&mut self, cycles: &mut Cycles) -> Addr24 {
        let addr = self.load::<u8>();
        if self.cpu().regs.dp & 0xff > 0 {
            *cycles += 1
        }
        self.read(Addr24::new(0, self.cpu().regs.dp.wrapping_add(addr.into())))
    }

    fn load_dp_indexed_v(&mut self, cycles: &mut Cycles, val: u16) -> Addr24 {
        let addr = self.load::<u8>();
        if self.cpu().regs.dp & 0xff > 0 {
            *cycles += 1
        }
        Addr24::new(
            0,
            self.cpu()
                .regs
                .dp
                .wrapping_add(addr.into())
                .wrapping_add(val),
        )
    }

    /// DP Indexed, X
    pub fn load_dp_indexed_x(&mut self, cycles: &mut Cycles) -> Addr24 {
        self.load_dp_indexed_v(
            cycles,
            if self.cpu().is_idx8() {
                self.cpu().regs.x & 0xff
            } else {
                self.cpu().regs.x
            },
        )
    }

    /// DP Indexed, Y
    pub fn load_dp_indexed_y(&mut self, cycles: &mut Cycles) -> Addr24 {
        self.load_dp_indexed_v(
            cycles,
            if self.cpu().is_idx8() {
                self.cpu().regs.y & 0xff
            } else {
                self.cpu().regs.y
            },
        )
    }

    /// Absolute Long Indexed, X
    pub fn load_long_indexed_x(&mut self) -> Addr24 {
        let Addr24 { mut bank, addr } = self.load::<Addr24>();
        let x = if self.cpu().is_idx8() {
            self.cpu().regs.x8().into()
        } else {
            self.cpu().regs.x
        };
        let (addr, ov) = x.overflowing_add(addr);
        if ov {
            bank = bank.wrapping_add(1)
        }
        Addr24::new(bank, addr)
    }

    /// Direct Page
    pub fn load_direct(&mut self, cycles: &mut Cycles) -> Addr24 {
        let val = self.load::<u8>();
        if self.cpu().regs.dp & 0xff > 0 {
            *cycles += 1
        }
        Addr24::new(0, self.cpu().regs.dp.wrapping_add(val.into()))
    }

    /// DP Indexed Indirect, X
    pub fn load_dp_indexed_indirect_x(&mut self, cycles: &mut Cycles) -> Addr24 {
        let val = self.load::<u8>();
        if self.cpu().regs.dp & 0xff > 0 {
            *cycles += 1
        }
        let addr = self
            .cpu()
            .regs
            .dp
            .wrapping_add(if self.cpu().is_idx8() {
                self.cpu().regs.x8().into()
            } else {
                self.cpu().regs.x
            })
            .wrapping_add(val.into());
        let addr = self.read(Addr24::new(0, addr));
        self.cpu().get_data_addr(addr)
    }

    /// DP Indirect Long Indexed, Y
    pub fn load_indirect_long_indexed_y(&mut self, cycles: &mut Cycles) -> Addr24 {
        let addr = self.load::<u8>();
        if self.cpu().regs.dp & 0xff > 0 {
            *cycles += 1
        }
        let addr =
            self.read::<Addr24>(Addr24::new(0, self.cpu().regs.dp.wrapping_add(addr.into())));
        let y = if self.cpu().is_idx8() {
            self.cpu().regs.y8().into()
        } else {
            self.cpu().regs.y
        };
        let (new_addr, ov) = addr.addr.overflowing_add(y);
        if ov {
            Addr24::new(addr.bank.wrapping_add(1), new_addr)
        } else {
            Addr24::new(addr.bank, new_addr)
        }
    }

    /// DP Indirect Indexed, Y
    pub fn load_indirect_indexed_y<const BC: bool>(&mut self, cycles: &mut Cycles) -> Addr24 {
        let addr = u16::from(self.load::<u8>());
        if self.cpu().regs.dp & 0xff > 0 {
            *cycles += 1
        }
        let addr = addr.wrapping_add(self.cpu().regs.dp);
        let addr = self.read::<u16>(Addr24::new(0, addr));
        let y = if self.cpu().is_idx8() {
            self.cpu().regs.y & 0xff
        } else {
            self.cpu().regs.y
        };
        let new_addr = addr.wrapping_add(y);
        if BC && (!self.cpu().is_idx8() || new_addr & 0xff00 != addr & 0xff00) {
            *cycles += 1
        }
        self.cpu().get_data_addr(new_addr)
    }

    /// Absolute Indexed Indirect
    pub fn load_indexed_indirect(&mut self) -> Addr24 {
        let x = if self.cpu().is_idx8() {
            self.cpu().regs.x8().into()
        } else {
            self.cpu().regs.x
        };
        let addr = self.load::<u16>().wrapping_add(x);
        let addr = Addr24::new(self.cpu().regs.pc.bank, addr);
        Addr24::new(self.cpu().regs.pc.bank, self.read(addr))
    }

    /// Stack Relative
    pub fn load_stack_relative(&mut self) -> Addr24 {
        let addr = self.load::<u8>();
        Addr24::new(0, self.cpu().regs.sp.wrapping_add(addr.into()))
    }

    /// SR Indirect Indexed, Y
    pub fn load_sr_indirect_indexed_y(&mut self) -> Addr24 {
        let indirect = self.cpu().regs.sp.wrapping_add(self.load::<u8>().into());
        let addr = self.read::<u16>(Addr24::new(0, indirect));
        let y = if self.cpu().is_idx8() {
            self.cpu().regs.y8().into()
        } else {
            self.cpu().regs.y
        };
        self.cpu().get_data_addr(addr.wrapping_add(y))
    }

    pub fn interrupt_instruction<
        const NATIVE_VECTOR: u16,
        const EMULATION_VECTOR: u16,
        const BREAK_FLAG: bool,
    >(
        &mut self,
        cycles: &mut Cycles,
    ) {
        let _ = self.load::<u8>();
        let (pushed_status, vector) = if !self.cpu().regs.is_emulation {
            *cycles += 1;
            self.push(self.cpu().regs.pc.bank);
            (0, NATIVE_VECTOR)
        } else if BREAK_FLAG {
            (Status::BREAK.0, EMULATION_VECTOR)
        } else {
            (0, EMULATION_VECTOR)
        };
        self.push(self.cpu().regs.pc.addr);
        self.push(self.cpu().regs.status.0 | pushed_status);
        let s = (self.cpu().regs.status | Status::IRQ_DISABLE) & !Status::DECIMAL;
        self.cpu_mut().regs.status = s;
        self.cpu_mut().regs.pc = Addr24::new(0, self.read(Addr24::new(0, vector)));
    }

    pub fn dispatch_instruction_with(&mut self, start_addr: Addr24, op: u8) -> Cycles {
        let mut cycles = CYCLES[op as usize];
        match op {
            0x00 => {
                // BRK - Break
                self.interrupt_instruction::<0xffe6, 0xfffe, true>(&mut cycles)
            }
            0x01 => {
                // ORA - Or A with DP Indexed Indirect, X
                let addr = self.load_dp_indexed_indirect_x(&mut cycles);
                self.ora(addr, &mut cycles)
            }
            0x02 => {
                // COP - Co-Processor Enable
                self.interrupt_instruction::<0xffe4, 0xfff4, false>(&mut cycles)
            }
            0x03 => {
                // ORA - Or A with Stack Relative
                let addr = self.load_stack_relative();
                self.ora(addr, &mut cycles)
            }
            0x04 => {
                // TSB - Test and set Bits from Direct Page in A
                let addr = self.load_direct(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr);
                    let a = self.cpu().regs.a8();
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::ZERO, a & val == 0);
                    self.write(addr, val | a)
                } else {
                    let val = self.read::<u16>(addr);
                    let a = self.cpu().regs.a;
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::ZERO, a & val == 0);
                    self.write(addr, val | a);
                    cycles += 2
                }
            }
            0x05 => {
                // ORA - Or A with direct page
                let addr = self.load_direct(&mut cycles);
                self.ora(addr, &mut cycles)
            }
            0x06 => {
                // ASL - Arithmetic left shift on DP
                let addr = self.load_direct(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr);
                    let newval = val << 1;
                    self.write(addr, newval);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val >= 0x80);
                    self.cpu_mut().update_nz8(newval);
                } else {
                    let val = self.read::<u16>(addr);
                    let newval = val << 1;
                    self.write(addr, newval);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val >= 0x8000);
                    self.cpu_mut().update_nz16(newval);
                    cycles += 2
                }
            }
            0x07 => {
                // ORA - Or A with DP Indirect Long
                let addr = self.load_dp_indirect_long(&mut cycles);
                self.ora(addr, &mut cycles)
            }
            0x08 => {
                // PHP - Push Status Register
                self.push(self.cpu().regs.status.0)
            }
            0x09 => {
                // ORA - Or A with immediate value
                if self.cpu().is_reg8() {
                    let val = self.load::<u8>() | self.cpu().regs.a8();
                    self.cpu_mut().regs.set_a8(val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    let val = self.load::<u16>() | self.cpu().regs.a;
                    self.cpu_mut().regs.a = val;
                    self.cpu_mut().update_nz16(val);
                    cycles += 1
                }
            }
            0x0a => {
                // ASL - Arithmetic left shift on A
                if self.cpu().is_reg8() {
                    let val = self.cpu().regs.a8();
                    let newval = val << 1;
                    self.cpu_mut().regs.set_a8(newval);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val >= 0x80);
                    self.cpu_mut().update_nz8(newval);
                } else {
                    let val = self.cpu().regs.a;
                    let newval = val << 1;
                    self.cpu_mut().regs.a = newval;
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val >= 0x8000);
                    self.cpu_mut().update_nz16(newval);
                }
            }
            0x0b => {
                // PHD - Push Direct Page
                self.push(self.cpu().regs.dp)
            }
            0x0c => {
                // TSB - Test and set Bits from Absolute
                let addr = self.load();
                let addr = self.cpu().get_data_addr(addr);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr);
                    let a = self.cpu().regs.a8();
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::ZERO, a & val == 0);
                    self.write(addr, val | a)
                } else {
                    let val = self.read::<u16>(addr);
                    let a = self.cpu().regs.a;
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::ZERO, a & val == 0);
                    self.write(addr, val | a);
                    cycles += 2
                }
            }
            0x0d => {
                // ORA - Or A with absolute value
                let addr = self.load::<u16>();
                let addr = self.cpu().get_data_addr(addr);
                self.ora(addr, &mut cycles)
            }
            0x0e => {
                // ASL - Arithmetic left shift on absolute value
                let addr = self.load::<u16>();
                let addr = self.cpu().get_data_addr(addr);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr);
                    let newval = val << 1;
                    self.write(addr, newval);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val >= 0x80);
                    self.cpu_mut().update_nz8(newval);
                } else {
                    let val = self.read::<u16>(addr);
                    let newval = val << 1;
                    self.write(addr, newval);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val >= 0x8000);
                    self.cpu_mut().update_nz16(newval);
                    cycles += 2
                }
            }
            0x0f => {
                // ORA - Or A with Absolute Long
                let addr = self.load();
                self.ora(addr, &mut cycles)
            }
            0x10 => {
                // BPL - Branch if Plus
                self.branch_near(!self.cpu().regs.status.has(Status::NEGATIVE), &mut cycles)
            }
            0x11 => {
                // ORA - Or A with DP Indirect Indexed, Y
                let addr = self.load_indirect_indexed_y::<true>(&mut cycles);
                self.ora(addr, &mut cycles)
            }
            0x12 => {
                // ORA - Or A with DP Indirect
                let addr = self.load_dp_indirect(&mut cycles);
                self.ora(addr, &mut cycles)
            }
            0x13 => {
                // ORA - Or A with SR Indirect Indexed, Y
                let addr = self.load_sr_indirect_indexed_y();
                self.ora(addr, &mut cycles)
            }
            0x14 => {
                // TRB - Test and Reset Bits from Direct Page in A
                let addr = self.load_direct(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr);
                    let a = self.cpu().regs.a8();
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::ZERO, a & val == 0);
                    self.write(addr, val & !a)
                } else {
                    let val = self.read::<u16>(addr);
                    let a = self.cpu().regs.a;
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::ZERO, a & val == 0);
                    self.write(addr, val & !a);
                    cycles += 2
                }
            }
            0x15 => {
                // ORA - Or A with DP Indexed,X
                let addr = self.load_dp_indexed_x(&mut cycles);
                self.ora(addr, &mut cycles)
            }
            0x16 => {
                // ASL - Arithmetic left shift on DP Indexed, X
                let addr = self.load_dp_indexed_x(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr);
                    let newval = val << 1;
                    self.write(addr, newval);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val >= 0x80);
                    self.cpu_mut().update_nz8(newval);
                } else {
                    let val = self.read::<u16>(addr);
                    let newval = val << 1;
                    self.write(addr, newval);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val >= 0x8000);
                    self.cpu_mut().update_nz16(newval);
                    cycles += 2
                }
            }
            0x17 => {
                // ORA - Or A with DP Indirect Long Indexed, Y
                let addr = self.load_indirect_long_indexed_y(&mut cycles);
                self.ora(addr, &mut cycles)
            }
            0x18 => {
                // CLC - Clear the Carry Flag
                self.cpu_mut().regs.status &= !Status::CARRY;
            }
            0x19 => {
                // ORA - Or A with Absolute Indexed, Y
                let addr = self.load_indexed_y::<true>(&mut cycles);
                self.ora(addr, &mut cycles)
            }
            0x1a => {
                // INC/INA - Increment A
                if self.cpu().is_reg8() {
                    let a = self.cpu().regs.a8().wrapping_add(1);
                    self.cpu_mut().regs.set_a8(a);
                    self.cpu_mut().update_nz8(a)
                } else {
                    let a = self.cpu().regs.a.wrapping_add(1);
                    self.cpu_mut().regs.a = a;
                    self.cpu_mut().update_nz16(a)
                }
            }
            0x1b => {
                // TCS - Transfer A to SP
                self.cpu_mut().regs.sp = self.cpu().regs.a
            }
            0x1c => {
                // TRB - Test and Reset Bits from Absolute in A
                let addr = self.load::<u16>();
                let addr = self.cpu().get_data_addr(addr);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr);
                    let a = self.cpu().regs.a8();
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::ZERO, a & val == 0);
                    self.write(addr, val & !a)
                } else {
                    let val = self.read::<u16>(addr);
                    let a = self.cpu().regs.a;
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::ZERO, a & val == 0);
                    self.write(addr, val & !a);
                    cycles += 2
                }
            }
            0x1d => {
                // ORA - Or A with Absolute Indexed, X
                let addr = self.load_indexed_x::<true>(&mut cycles);
                self.ora(addr, &mut cycles)
            }
            0x1e => {
                // ASL - Arithmetic left shift on Absolute Indexed, X
                let addr = self.load_indexed_x::<false>(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr);
                    let newval = val << 1;
                    self.write(addr, newval);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val >= 0x80);
                    self.cpu_mut().update_nz8(newval);
                } else {
                    let val = self.read::<u16>(addr);
                    let newval = val << 1;
                    self.write(addr, newval);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val >= 0x8000);
                    self.cpu_mut().update_nz16(newval);
                    cycles += 2
                }
            }
            0x1f => {
                // ORA - Or A with Absolute Long Indexed, X
                let addr = self.load_long_indexed_x();
                self.ora(addr, &mut cycles)
            }
            0x20 => {
                // JSR - Jump to Subroutine
                self.push(start_addr.addr.wrapping_add(2));
                let new_addr = self.load::<u16>();
                self.cpu_mut().regs.pc.addr = new_addr;
            }
            0x21 => {
                // AND - And A with DP Indexed Indirect, X
                let addr = self.load_dp_indexed_indirect_x(&mut cycles);
                self.and(addr, &mut cycles);
            }
            0x22 => {
                // JSR/JSL - Jump to Subroutine Long
                self.push(start_addr.bank);
                self.push(start_addr.addr.wrapping_add(3));
                let new_addr = self.load::<Addr24>();
                self.cpu_mut().regs.pc = new_addr;
            }
            0x23 => {
                // AND - And A with Stack Relative
                let addr = self.load_stack_relative();
                self.and(addr, &mut cycles);
            }
            0x24 => {
                // BIT - Test Bit from absolute index
                let addr = self.load_direct(&mut cycles);
                self.test_bit(addr, &mut cycles)
            }
            0x25 => {
                // AND - And A with direct page
                let addr = self.load_direct(&mut cycles);
                self.and(addr, &mut cycles);
            }
            0x26 => {
                // ROL - Rotate Direct Page left
                let addr = self.load_direct(&mut cycles);
                self.rotate_left(addr, &mut cycles)
            }
            0x27 => {
                // AND - And A with DP Indirect Long
                let addr = self.load_dp_indirect_long(&mut cycles);
                self.and(addr, &mut cycles);
            }
            0x28 => {
                // PLP - Pull status
                self.cpu_mut().regs.status = Status(self.pull::<u8>());
                self.cpu_mut().update_status();
            }
            0x29 => {
                // AND - bitwise and A with immediate value
                if self.cpu().is_reg8() {
                    let value = self.cpu().regs.a8() & self.load::<u8>();
                    self.cpu_mut().regs.set_a8(value);
                    self.cpu_mut().update_nz8(value);
                } else {
                    let value = self.cpu().regs.a & self.load::<u16>();
                    self.cpu_mut().regs.a = value;
                    self.cpu_mut().update_nz16(value);
                    cycles += 1
                }
            }
            0x2a => {
                // ROL - Rotate A left
                if self.cpu().is_reg8() {
                    let val = self.cpu().regs.a8();
                    let res = self.cpu().regs.status.has(Status::CARRY) as u8 | (val << 1);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val & 0x80 > 0);
                    self.cpu_mut().update_nz8(res);
                    self.cpu_mut().regs.set_a8(res);
                } else {
                    let res =
                        self.cpu().regs.status.has(Status::CARRY) as u16 | (self.cpu().regs.a << 1);
                    let s = self.cpu().regs.a & 0x8000 > 0;
                    self.cpu_mut().regs.status.set_if(Status::CARRY, s);
                    self.cpu_mut().update_nz16(res);
                    self.cpu_mut().regs.a = res;
                }
            }
            0x2b => {
                // PLD - Pull Direct Page Register
                let dp = self.pull();
                self.cpu_mut().regs.dp = dp;
                self.cpu_mut().update_nz16(dp);
            }
            0x2c => {
                // BIT - Test Bit from absolute index
                let addr = self.load::<u16>();
                let addr = self.cpu().get_data_addr(addr);
                self.test_bit(addr, &mut cycles)
            }
            0x2d => {
                // AND - AND absolute on A
                let addr = self.load();
                let addr = self.cpu().get_data_addr(addr);
                self.and(addr, &mut cycles);
            }
            0x2e => {
                // ROL - Rotate Absolute left
                let addr = self.load::<u16>();
                let addr = self.cpu().get_data_addr(addr);
                self.rotate_left(addr, &mut cycles)
            }
            0x2f => {
                // AND - And A with Absolute Long
                let addr = self.load::<Addr24>();
                self.and(addr, &mut cycles);
            }
            0x30 => {
                // BMI - Branch if Negative Flag set
                self.branch_near(self.cpu().regs.status.has(Status::NEGATIVE), &mut cycles)
            }
            0x31 => {
                // AND - And A with DP Indirect Indexed, Y
                let addr = self.load_indirect_indexed_y::<true>(&mut cycles);
                self.and(addr, &mut cycles);
            }
            0x32 => {
                // AND - And A with DP Indirect
                let addr = self.load_dp_indirect(&mut cycles);
                self.and(addr, &mut cycles);
            }
            0x33 => {
                // AND - And A with SR Indirect Indexed, Y
                let addr = self.load_sr_indirect_indexed_y();
                self.and(addr, &mut cycles);
            }
            0x34 => {
                // BIT - Test Bit from DP Indexed, X index
                let addr = self.load_dp_indexed_x(&mut cycles);
                self.test_bit(addr, &mut cycles)
            }
            0x35 => {
                // AND - And A with DP Indexed, X
                let addr = self.load_dp_indexed_x(&mut cycles);
                self.and(addr, &mut cycles);
            }
            0x36 => {
                // ROL - Rotate DP Indexed, X left
                let addr = self.load_dp_indexed_x(&mut cycles);
                self.rotate_left(addr, &mut cycles)
            }
            0x37 => {
                // AND - And A with DP Indirect Long Indexed, Y
                let addr = self.load_indirect_long_indexed_y(&mut cycles);
                self.and(addr, &mut cycles);
            }
            0x38 => {
                // SEC - Set Carry Flag
                self.cpu_mut().regs.status |= Status::CARRY
            }
            0x39 => {
                // AND - And A with Absolute Indexed, Y
                let addr = self.load_indexed_y::<true>(&mut cycles);
                self.and(addr, &mut cycles);
            }
            0x3a => {
                // DEC/DEA - Decrement A
                if self.cpu().is_reg8() {
                    let a = self.cpu().regs.a8().wrapping_sub(1);
                    self.cpu_mut().regs.set_a8(a);
                    self.cpu_mut().update_nz8(a)
                } else {
                    let a = self.cpu().regs.a.wrapping_sub(1);
                    self.cpu_mut().regs.a = a;
                    self.cpu_mut().update_nz16(a)
                }
            }
            0x3b => {
                // TSC - Transfer SP to A
                let a = self.cpu().regs.sp;
                self.cpu_mut().regs.a = a;
                self.cpu_mut().update_nz16(a);
            }
            0x3c => {
                // BIT - Test Bit from Absolute Indexed, X
                let addr = self.load_indexed_x::<true>(&mut cycles);
                self.test_bit(addr, &mut cycles)
            }
            0x3d => {
                // AND - And A with Absolute Indexed, X
                let addr = self.load_indexed_x::<true>(&mut cycles);
                self.and(addr, &mut cycles);
            }
            0x3e => {
                // ROL - Rotate Absolute Indexed, X left
                let addr = self.load_indexed_x::<false>(&mut cycles);
                self.rotate_left(addr, &mut cycles)
            }
            0x3f => {
                // AND - And A with Absolute Long Indexed, X
                let addr = self.load_long_indexed_x();
                self.and(addr, &mut cycles);
            }
            0x40 => {
                // RTI - Return from interrupt
                self.cpu_mut().in_nmi = false;
                self.cpu_mut().regs.status.0 = self.pull();
                self.cpu_mut().update_status();
                self.cpu_mut().regs.pc.addr = self.pull();
                if !self.cpu().regs.is_emulation {
                    self.cpu_mut().regs.pc.bank = self.pull();
                    cycles += 1
                }
            }
            0x41 => {
                // EOR - XOR DP Indexed Indirect, X on A
                let addr = self.load_dp_indexed_indirect_x(&mut cycles);
                self.exclusive_or(addr, &mut cycles)
            }
            0x42 => {
                // WDM - a worse NOP
                let _ = self.load::<u8>();
            }
            0x43 => {
                // EOR - XOR SR on A
                let addr = self.load_stack_relative();
                self.exclusive_or(addr, &mut cycles)
            }
            0x44 => {
                // MVP - Block Move Positive
                self.block_move::<0xffff>()
            }
            0x45 => {
                // EOR - XOR DP on A
                let addr = self.load_direct(&mut cycles);
                self.exclusive_or(addr, &mut cycles)
            }
            0x46 => {
                // LSR - SHR on Direct Page
                let addr = self.load_direct(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val & 1 > 0);
                    let val = val >> 1;
                    self.write(addr, val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    let val = self.read::<u16>(addr);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val & 1 > 0);
                    let val = val >> 1;
                    self.write(addr, val);
                    self.cpu_mut().update_nz16(val);
                    cycles += 2
                }
            }
            0x47 => {
                // EOR - XOR DP Indirect Long on A
                let addr = self.load_dp_indirect_long(&mut cycles);
                self.exclusive_or(addr, &mut cycles)
            }
            0x48 => {
                // PHA - Push A
                if self.cpu().is_reg8() {
                    self.push(self.cpu().regs.a8())
                } else {
                    self.push(self.cpu().regs.a);
                    cycles += 1
                }
            }
            0x49 => {
                // EOR - XOR A with immediate value
                if self.cpu().is_reg8() {
                    let val = self.load::<u8>() ^ self.cpu().regs.a8();
                    self.cpu_mut().regs.set_a8(val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    let val = self.load::<u16>() ^ self.cpu().regs.a;
                    self.cpu_mut().regs.a = val;
                    self.cpu_mut().update_nz16(val);
                    cycles += 1
                }
            }
            0x4a => {
                // LSR - SHR on A
                if self.cpu().is_reg8() {
                    let val = self.cpu().regs.a8();
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val & 1 > 0);
                    let val = val >> 1;
                    self.cpu_mut().regs.set_a8(val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    let a = self.cpu().regs.a & 1 > 0;
                    self.cpu_mut().regs.status.set_if(Status::CARRY, a);
                    let a = self.cpu().regs.a >> 1;
                    self.cpu_mut().regs.a = a;
                    self.cpu_mut().update_nz16(a);
                }
            }
            0x4b => {
                // PHK - Push PC Bank
                self.push(self.cpu().regs.pc.bank)
            }
            0x4c => {
                // JMP - Jump absolute
                self.cpu_mut().regs.pc.addr = self.load()
            }
            0x4d => {
                // EOR - XOR absolute on A
                let addr = self.load();
                let addr = self.cpu().get_data_addr(addr);
                self.exclusive_or(addr, &mut cycles)
            }
            0x4e => {
                // LSR - SHR on absolute
                let addr = self.load::<u16>();
                let addr = self.cpu().get_data_addr(addr);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val & 1 > 0);
                    let val = val >> 1;
                    self.write(addr, val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    let val = self.read::<u16>(addr);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val & 1 > 0);
                    let val = val >> 1;
                    self.write(addr, val);
                    self.cpu_mut().update_nz16(val);
                    cycles += 2
                }
            }
            0x4f => {
                // EOR - XOR Absolute Long on A
                let addr: Addr24 = self.load();
                self.exclusive_or(addr, &mut cycles)
            }
            0x50 => {
                // BVC - Branch if Overflow is set
                self.branch_near(!self.cpu().regs.status.has(Status::OVERFLOW), &mut cycles)
            }
            0x51 => {
                // EOR - XOR DP Indirect Indexed, Y on A
                let addr = self.load_indirect_indexed_y::<true>(&mut cycles);
                self.exclusive_or(addr, &mut cycles)
            }
            0x52 => {
                // EOR - XOR DP Indirect on A
                let addr = self.load_dp_indirect(&mut cycles);
                self.exclusive_or(addr, &mut cycles)
            }
            0x53 => {
                // EOR - XOR DP Indirect on A
                let addr = self.load_sr_indirect_indexed_y();
                self.exclusive_or(addr, &mut cycles)
            }
            0x54 => {
                // MVN - Block Move Negative
                self.block_move::<1>()
            }
            0x55 => {
                // EOR - XOR DP Indexed, X on A
                let addr = self.load_dp_indexed_x(&mut cycles);
                self.exclusive_or(addr, &mut cycles)
            }
            0x56 => {
                // LSR - SHR on DP Indexed, X
                let addr = self.load_dp_indexed_x(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val & 1 > 0);
                    let val = val >> 1;
                    self.write(addr, val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    let val = self.read::<u16>(addr);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val & 1 > 0);
                    let val = val >> 1;
                    self.write(addr, val);
                    self.cpu_mut().update_nz16(val);
                    cycles += 2
                }
            }
            0x57 => {
                // EOR - XOR DP Indirect Long Indexed, Y on A
                let addr = self.load_indirect_long_indexed_y(&mut cycles);
                self.exclusive_or(addr, &mut cycles)
            }
            0x58 => {
                // CLI - Clear IRQ_DISABLE
                self.cpu_mut().regs.status &= !Status::IRQ_DISABLE
            }
            0x59 => {
                // EOR - XOR Absolute Indexed, Y on A
                let addr = self.load_indexed_y::<true>(&mut cycles);
                self.exclusive_or(addr, &mut cycles)
            }
            0x5a => {
                // PHY - Push Y
                if self.cpu().is_idx8() {
                    self.push(self.cpu().regs.y8())
                } else {
                    self.push(self.cpu().regs.y);
                    cycles += 1
                }
            }
            0x5b => {
                // TCD - Transfer A to DP
                let a = self.cpu().regs.a;
                self.cpu_mut().regs.dp = a;
                self.cpu_mut().update_nz16(a)
            }
            0x5c => {
                // JMP/JML - Jump absolute Long
                self.cpu_mut().regs.pc = self.load::<Addr24>();
            }
            0x5d => {
                // EOR - XOR Absolute Indexed, X on A
                let addr = self.load_indexed_x::<true>(&mut cycles);
                self.exclusive_or(addr, &mut cycles)
            }
            0x5e => {
                // LSR - SHR on Absolute Indexed, X
                let addr = self.load_indexed_x::<false>(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val & 1 > 0);
                    let val = val >> 1;
                    self.write(addr, val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    let val = self.read::<u16>(addr);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val & 1 > 0);
                    let val = val >> 1;
                    self.write(addr, val);
                    self.cpu_mut().update_nz16(val);
                    cycles += 2
                }
            }
            0x5f => {
                // EOR - XOR Absolute Long Indexed, X on A
                let addr = self.load_long_indexed_x();
                self.exclusive_or(addr, &mut cycles)
            }
            0x60 => {
                // RTS - Return from subroutine
                self.cpu_mut().regs.pc.addr = 1u16.wrapping_add(self.pull());
            }
            0x61 => {
                // ADC - DP Indexed Indirect, X Add with Carry
                let addr = self.load_dp_indexed_indirect_x(&mut cycles);
                self.add_carry_memory(addr, &mut cycles)
            }
            0x62 => {
                // PER - Push PC + imm
                let val = self.load::<u16>();
                let val = self.cpu().regs.pc.addr.wrapping_add(val);
                self.push(val)
            }
            0x63 => {
                // ADC - Stack Relative Add with Carry
                let addr = self.load_stack_relative();
                self.add_carry_memory(addr, &mut cycles)
            }
            0x64 => {
                // STZ - Store Zero to memory
                let addr = self.load_direct(&mut cycles);
                self.store_zero(addr, &mut cycles)
            }
            0x65 => {
                // ADC - DP Add with Carry
                let addr = self.load_direct(&mut cycles);
                self.add_carry_memory(addr, &mut cycles)
            }
            0x66 => {
                // ROR - Rotate Direct Page right
                let addr = self.load_direct(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr);
                    let res = ((self.cpu().regs.status.has(Status::CARRY) as u8) << 7) | (val >> 1);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val & 1 > 0);
                    self.cpu_mut().update_nz8(res);
                    self.write(addr, res);
                } else {
                    let val = self.read::<u16>(addr);
                    let res =
                        ((self.cpu().regs.status.has(Status::CARRY) as u16) << 15) | (val >> 1);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val & 1 > 0);
                    self.cpu_mut().update_nz16(res);
                    self.write(addr, res);
                    cycles += 2
                }
            }
            0x67 => {
                // ADC - Add DP Indirect Long with Carry
                let addr = self.load_dp_indirect_long(&mut cycles);
                self.add_carry_memory(addr, &mut cycles)
            }
            0x68 => {
                // PLA - Pull A
                if self.cpu().is_reg8() {
                    let a = self.pull();
                    self.cpu_mut().regs.set_a8(a);
                    self.cpu_mut().update_nz8(a);
                } else {
                    let a = self.pull();
                    self.cpu_mut().regs.a = a;
                    self.cpu_mut().update_nz16(a);
                    cycles += 1
                }
            }
            0x69 => {
                // ADC -  immediate Add with Carry
                if self.cpu().is_reg8() {
                    let op1 = self.load::<u8>();
                    self.add_carry8(op1);
                } else {
                    let op1 = self.load::<u16>();
                    self.add_carry16(op1);
                    cycles += 1;
                }
            }
            0x6a => {
                // ROR - Rotate A right
                if self.cpu().is_reg8() {
                    let val = self.cpu().regs.a8();
                    let res = ((self.cpu().regs.status.has(Status::CARRY) as u8) << 7) | (val >> 1);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val & 1 > 0);
                    self.cpu_mut().update_nz8(res);
                    self.cpu_mut().regs.set_a8(res);
                } else {
                    let res = ((self.cpu().regs.status.has(Status::CARRY) as u16) << 15)
                        | (self.cpu().regs.a >> 1);
                    let a = self.cpu().regs.a & 1 > 0;
                    self.cpu_mut().regs.status.set_if(Status::CARRY, a);
                    self.cpu_mut().update_nz16(res);
                    self.cpu_mut().regs.a = res;
                }
            }
            0x6b => {
                // RTL - Return from subroutine long
                self.cpu_mut().regs.pc = self.pull();
                self.cpu_mut().regs.pc.addr = self.cpu().regs.pc.addr.wrapping_add(1);
            }
            0x6c => {
                // JMP - Jump Absolute Indirect
                let addr = self.load::<u16>();
                let addr = self.read(Addr24::new(0, addr));
                self.cpu_mut().regs.pc.addr = addr;
            }
            0x6d => {
                // ADC - Add absolute with Carry
                let addr = self.load();
                let addr = self.cpu().get_data_addr(addr);
                self.add_carry_memory(addr, &mut cycles)
            }
            0x6e => {
                // ROR - Rotate Absolute right
                let addr = self.load::<u16>();
                let addr = self.cpu().get_data_addr(addr);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr);
                    let res = ((self.cpu().regs.status.has(Status::CARRY) as u8) << 7) | (val >> 1);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val & 1 > 0);
                    self.cpu_mut().update_nz8(res);
                    self.write(addr, res);
                } else {
                    let val = self.read::<u16>(addr);
                    let res =
                        ((self.cpu().regs.status.has(Status::CARRY) as u16) << 15) | (val >> 1);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val & 1 > 0);
                    self.cpu_mut().update_nz16(res);
                    self.write(addr, res);
                    cycles += 2
                }
            }
            0x6f => {
                // ADC - Add with Carry Absolute Long
                let addr = self.load::<Addr24>();
                self.add_carry_memory(addr, &mut cycles)
            }
            0x70 => {
                // BVS - Branch if Overflow is set
                self.branch_near(self.cpu().regs.status.has(Status::OVERFLOW), &mut cycles)
            }
            0x71 => {
                // ADC - Add with Carry DP Indirect Indexed, Y
                let addr = self.load_indirect_indexed_y::<true>(&mut cycles);
                self.add_carry_memory(addr, &mut cycles)
            }
            0x72 => {
                // ADC - DP Indirect Add with Carry
                let addr = self.load_dp_indirect(&mut cycles);
                self.add_carry_memory(addr, &mut cycles)
            }
            0x73 => {
                // ADC - SR Indirect Indexed, Y Add with Carry
                let addr = self.load_sr_indirect_indexed_y();
                self.add_carry_memory(addr, &mut cycles)
            }
            0x74 => {
                // STZ - Store Zero to DP X indexed memory
                let addr = self.load_dp_indexed_x(&mut cycles);
                self.store_zero(addr, &mut cycles)
            }
            0x75 => {
                // ADC - Add with Carry DP Indexed, X
                let addr = self.load_dp_indexed_x(&mut cycles);
                self.add_carry_memory(addr, &mut cycles)
            }
            0x76 => {
                // ROR - Rotate DP Indexed, X right
                let addr = self.load_dp_indexed_x(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr);
                    let res = ((self.cpu().regs.status.has(Status::CARRY) as u8) << 7) | (val >> 1);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val & 1 > 0);
                    self.cpu_mut().update_nz8(res);
                    self.write(addr, res);
                } else {
                    let val = self.read::<u16>(addr);
                    let res =
                        ((self.cpu().regs.status.has(Status::CARRY) as u16) << 15) | (val >> 1);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val & 1 > 0);
                    self.cpu_mut().update_nz16(res);
                    self.write(addr, res);
                    cycles += 2
                }
            }
            0x77 => {
                // ADC - Add with Carry DP Indirect Long Indexed, Y
                let addr = self.load_indirect_long_indexed_y(&mut cycles);
                self.add_carry_memory(addr, &mut cycles)
            }
            0x78 => {
                // SEI - Set the Interrupt Disable flag
                self.cpu_mut().regs.status |= Status::IRQ_DISABLE
            }
            0x79 => {
                // ADC - Add with Carry Absolute Indexed, Y
                let addr = self.load_indexed_y::<true>(&mut cycles);
                self.add_carry_memory(addr, &mut cycles)
            }
            0x7a => {
                // PLY - Pull Y
                if self.cpu().is_idx8() {
                    let y = self.pull();
                    self.cpu_mut().regs.set_y8(y);
                    self.cpu_mut().update_nz8(y);
                } else {
                    let y = self.pull();
                    self.cpu_mut().regs.y = y;
                    self.cpu_mut().update_nz16(y);
                    cycles += 1
                }
            }
            0x7b => {
                // TDC - Transfer DP register to A
                let a = self.cpu().regs.dp;
                self.cpu_mut().regs.a = a;
                self.cpu_mut().update_nz16(a)
            }
            0x7c => {
                // JMP - Jump Absolute Indexed Indirect
                let addr = self.load_indexed_indirect();
                self.cpu_mut().regs.pc = addr;
            }
            0x7d => {
                // ADC - Add with Carry
                let addr = self.load_indexed_x::<true>(&mut cycles);
                self.add_carry_memory(addr, &mut cycles)
            }
            0x7e => {
                // ROR - Rotate Absolute Indexed, X right
                let addr = self.load_indexed_x::<false>(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr);
                    let res = ((self.cpu().regs.status.has(Status::CARRY) as u8) << 7) | (val >> 1);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val & 1 > 0);
                    self.cpu_mut().update_nz8(res);
                    self.write(addr, res);
                } else {
                    let val = self.read::<u16>(addr);
                    let res =
                        ((self.cpu().regs.status.has(Status::CARRY) as u16) << 15) | (val >> 1);
                    self.cpu_mut()
                        .regs
                        .status
                        .set_if(Status::CARRY, val & 1 > 0);
                    self.cpu_mut().update_nz16(res);
                    self.write(addr, res);
                    cycles += 2
                }
            }
            0x7f => {
                // ADC - Add Absolute Long Indexed, X with Carry
                let addr = self.load_long_indexed_x();
                self.add_carry_memory(addr, &mut cycles)
            }
            0x80 => {
                // BRA - Branch always
                self.branch_near(true, &mut cycles);
            }
            0x81 => {
                // STA - Store A to DP Indexed Indirect, X
                let addr = self.load_dp_indexed_indirect_x(&mut cycles);
                if self.cpu().is_reg8() {
                    self.write::<u8>(addr, self.cpu().regs.a8());
                } else {
                    self.write::<u16>(addr, self.cpu().regs.a);
                    cycles += 1;
                }
            }
            0x82 => {
                // BRL - Branch always Program Counter Relative Long
                let rel = self.load::<u16>();
                self.cpu_mut().regs.pc.addr = self.cpu().regs.pc.addr.wrapping_add(rel);
            }
            0x83 => {
                // STA - Store A to Stack Relative
                let addr = self.load_stack_relative();
                if self.cpu().is_reg8() {
                    self.write::<u8>(addr, self.cpu().regs.a8());
                } else {
                    self.write::<u16>(addr, self.cpu().regs.a);
                    cycles += 1;
                }
            }
            0x84 => {
                // STY - Store Y to direct page
                let addr = self.load_direct(&mut cycles);
                if self.cpu().is_idx8() {
                    self.write::<u8>(addr, self.cpu().regs.y8());
                } else {
                    self.write::<u16>(addr, self.cpu().regs.y);
                    cycles += 1;
                }
            }
            0x85 => {
                // STA - Store A to direct page
                let addr = self.load_direct(&mut cycles);
                if self.cpu().is_reg8() {
                    self.write::<u8>(addr, self.cpu().regs.a8());
                } else {
                    self.write::<u16>(addr, self.cpu().regs.a);
                    cycles += 1;
                }
            }
            0x86 => {
                // STX - Store X to direct page
                let addr = self.load_direct(&mut cycles);
                if self.cpu().is_idx8() {
                    self.write::<u8>(addr, self.cpu().regs.x8());
                } else {
                    self.write::<u16>(addr, self.cpu().regs.x);
                    cycles += 1;
                }
            }
            0x87 => {
                // STA - Store A to DP Inirect long
                let addr = self.load_dp_indirect_long(&mut cycles);
                if self.cpu().is_reg8() {
                    self.write(addr, self.cpu().regs.a8())
                } else {
                    self.write(addr, self.cpu().regs.a);
                    cycles += 1
                }
            }
            0x88 => {
                // DEY - Decrement Y
                if self.cpu().is_idx8() {
                    let y = self.cpu().regs.y8().wrapping_sub(1);
                    self.cpu_mut().regs.set_y8(y);
                    self.cpu_mut().update_nz8(y);
                } else {
                    let y = self.cpu().regs.y.wrapping_sub(1);
                    self.cpu_mut().regs.y = y;
                    self.cpu_mut().update_nz16(y);
                }
            }
            0x89 => {
                // BIT - Test immediate bit
                if self.cpu().is_reg8() {
                    let val = self.load::<u8>();
                    let a = self.cpu().regs.a8() & val == 0;
                    self.cpu_mut().regs.status.set_if(Status::ZERO, a);
                } else {
                    let val = self.load::<u16>();
                    let a = self.cpu().regs.a & val == 0;
                    self.cpu_mut().regs.status.set_if(Status::ZERO, a);
                    cycles += 1
                }
            }
            0x8a => {
                // TXA - Transfer X to A
                if self.cpu().is_reg8() {
                    let val = self.cpu().regs.x8();
                    self.cpu_mut().regs.set_a8(val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    let x = if self.cpu().is_idx8() {
                        self.cpu().regs.x8().into()
                    } else {
                        self.cpu().regs.x
                    };
                    self.cpu_mut().regs.a = x;
                    self.cpu_mut().update_nz16(x)
                }
            }
            0x8b => {
                // PHB - Push Data Bank
                self.push(self.cpu().regs.db)
            }
            0x8c => {
                // STY - Store Y to absolute address
                let addr = self.load::<u16>();
                let addr = self.cpu().get_data_addr(addr);
                if self.cpu().is_idx8() {
                    self.write::<u8>(addr, self.cpu().regs.y8());
                } else {
                    self.write::<u16>(addr, self.cpu().regs.y);
                    cycles += 1;
                }
            }
            0x8d => {
                // STA - Store A to absolute address
                let addr = self.load::<u16>();
                let addr = self.cpu().get_data_addr(addr);
                if self.cpu().is_reg8() {
                    self.write::<u8>(addr, self.cpu().regs.a8());
                } else {
                    self.write::<u16>(addr, self.cpu().regs.a);
                    cycles += 1;
                }
            }
            0x8e => {
                // STX - Store X to absolute address
                let addr = self.load::<u16>();
                let addr = self.cpu().get_data_addr(addr);
                if self.cpu().is_idx8() {
                    self.write::<u8>(addr, self.cpu().regs.x8());
                } else {
                    self.write::<u16>(addr, self.cpu().regs.x);
                    cycles += 1;
                }
            }
            0x8f => {
                // STA - Store A to absolute long address
                let addr = self.load::<Addr24>();
                if self.cpu().is_reg8() {
                    self.write::<u8>(addr, self.cpu().regs.a8());
                } else {
                    self.write::<u16>(addr, self.cpu().regs.a);
                    cycles += 1;
                }
            }
            0x90 => {
                // BCC/BLT - Branch if Carry Clear
                self.branch_near(!self.cpu().regs.status.has(Status::CARRY), &mut cycles)
            }
            0x91 => {
                // STA - Store A to DP Indirect Indexed, Y
                let addr = self.load_indirect_indexed_y::<false>(&mut cycles);
                if self.cpu().is_reg8() {
                    self.write::<u8>(addr, self.cpu().regs.a8());
                } else {
                    self.write::<u16>(addr, self.cpu().regs.a);
                    cycles += 1;
                }
            }
            0x92 => {
                // STA - Store A to DP Indirect
                let addr = self.load_dp_indirect(&mut cycles);
                if self.cpu().is_reg8() {
                    self.write::<u8>(addr, self.cpu().regs.a8());
                } else {
                    self.write::<u16>(addr, self.cpu().regs.a);
                    cycles += 1;
                }
            }
            0x93 => {
                // STA - Store A to Stack Relative
                let addr = self.load_sr_indirect_indexed_y();
                if self.cpu().is_reg8() {
                    self.write::<u8>(addr, self.cpu().regs.a8());
                } else {
                    self.write::<u16>(addr, self.cpu().regs.a);
                    cycles += 1;
                }
            }
            0x94 => {
                // STY - Store Y to DP Indexed, X
                let addr = self.load_dp_indexed_x(&mut cycles);
                if self.cpu().is_idx8() {
                    self.write::<u8>(addr, self.cpu().regs.y8());
                } else {
                    self.write::<u16>(addr, self.cpu().regs.y);
                    cycles += 1;
                }
            }
            0x95 => {
                // STA - Store A to DP Indexed, X
                let addr = self.load_dp_indexed_x(&mut cycles);
                if self.cpu().is_reg8() {
                    self.write::<u8>(addr, self.cpu().regs.a8());
                } else {
                    self.write::<u16>(addr, self.cpu().regs.a);
                    cycles += 1;
                }
            }
            0x96 => {
                // STX - Store X to DP Indexed,Y
                let addr = self.load_dp_indexed_y(&mut cycles);
                if self.cpu().is_idx8() {
                    self.write::<u8>(addr, self.cpu().regs.x8());
                } else {
                    self.write::<u16>(addr, self.cpu().regs.x);
                    cycles += 1;
                }
            }
            0x97 => {
                // STA - Store A to DP indirect long indexed, Y
                let addr = self.load_indirect_long_indexed_y(&mut cycles);
                if self.cpu().is_reg8() {
                    self.write::<u8>(addr, self.cpu().regs.a8());
                } else {
                    self.write::<u16>(addr, self.cpu().regs.a);
                    cycles += 1;
                }
            }
            0x98 => {
                // TYA - Transfer Y to A
                if self.cpu().is_reg8() {
                    let y = self.cpu().regs.y8();
                    self.cpu_mut().regs.set_a8(y);
                    self.cpu_mut().update_nz8(y)
                } else {
                    let a = self.cpu().regs.y;
                    self.cpu_mut().regs.a = a;
                    self.cpu_mut().update_nz16(a)
                }
            }
            0x99 => {
                // STA - Store A to absolute indexed Y
                let addr = self.load_indexed_y::<false>(&mut cycles);
                if self.cpu().is_reg8() {
                    self.write(addr, self.cpu().regs.a8());
                } else {
                    self.write(addr, self.cpu().regs.a);
                    cycles += 1
                }
            }
            0x9a => {
                // TXS - Transfer X to SP
                self.cpu_mut().regs.sp = self.cpu().regs.x
            }
            0x9b => {
                // TXY - Transfer X to Y
                if self.cpu().is_idx8() {
                    let x = self.cpu().regs.x8();
                    self.cpu_mut().regs.set_y8(x);
                    self.cpu_mut().update_nz8(x);
                } else {
                    self.cpu_mut().regs.y = self.cpu().regs.x;
                    let x = self.cpu().regs.x;
                    self.cpu_mut().update_nz16(x);
                }
            }
            0x9c => {
                // STZ - absolute addressing
                let addr = self.load::<u16>();
                self.store_zero(self.cpu().get_data_addr(addr), &mut cycles)
            }
            0x9d => {
                // STA - Store A to absolute indexed X
                let addr = self.load_indexed_x::<false>(&mut cycles);
                if self.cpu().is_reg8() {
                    self.write(addr, self.cpu().regs.a8());
                } else {
                    self.write(addr, self.cpu().regs.a);
                    cycles += 1
                }
            }
            0x9e => {
                // STZ - absoulte X indexed
                let addr = self.load_indexed_x::<false>(&mut cycles);
                self.store_zero(addr, &mut cycles)
            }
            0x9f => {
                // STA - Store absolute long indexed A to address
                let addr = self.load_long_indexed_x();
                if self.cpu().is_reg8() {
                    self.write::<u8>(addr, self.cpu().regs.a8());
                } else {
                    self.write::<u16>(addr, self.cpu().regs.a);
                    cycles += 1;
                }
            }
            0xa0 => {
                // LDY - Load immediate into Y
                if self.cpu().is_idx8() {
                    let y = self.load::<u8>();
                    self.cpu_mut().update_nz8(y);
                    self.cpu_mut().regs.set_y8(y);
                } else {
                    let y = self.load::<u16>();
                    self.cpu_mut().update_nz16(y);
                    self.cpu_mut().regs.y = y;
                    cycles += 1;
                }
            }
            0xa1 => {
                // LDA - Load DP Indexed Indirect, X into A
                let addr = self.load_dp_indexed_indirect_x(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read(addr);
                    self.cpu_mut().regs.set_a8(val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    let val = self.read(addr);
                    self.cpu_mut().regs.a = val;
                    self.cpu_mut().update_nz16(val);
                    cycles += 1;
                }
            }
            0xa2 => {
                // LDX - Load immediate into X
                if self.cpu().is_idx8() {
                    let x = self.load::<u8>();
                    self.cpu_mut().update_nz8(x);
                    self.cpu_mut().regs.set_x8(x);
                } else {
                    let x = self.load::<u16>();
                    self.cpu_mut().update_nz16(x);
                    self.cpu_mut().regs.x = x;
                    cycles += 1;
                }
            }
            0xa3 => {
                // LDA - Load Stack Relative into A
                let addr = self.load_stack_relative();
                if self.cpu().is_reg8() {
                    let val = self.read(addr);
                    self.cpu_mut().regs.set_a8(val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    let val = self.read(addr);
                    self.cpu_mut().regs.a = val;
                    self.cpu_mut().update_nz16(val);
                    cycles += 1;
                }
            }
            0xa4 => {
                // LDY - Load direct page into Y
                let addr = self.load_direct(&mut cycles);
                if self.cpu().is_idx8() {
                    let y = self.read::<u8>(addr);
                    self.cpu_mut().update_nz8(y);
                    self.cpu_mut().regs.set_y8(y);
                } else {
                    let y = self.read::<u16>(addr);
                    self.cpu_mut().update_nz16(y);
                    self.cpu_mut().regs.y = y;
                    cycles += 1;
                }
            }
            0xa5 => {
                // LDA - Load direct page to A
                let addr = self.load_direct(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read(addr);
                    self.cpu_mut().regs.set_a8(val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    let val = self.read(addr);
                    self.cpu_mut().regs.a = val;
                    self.cpu_mut().update_nz16(val);
                    cycles += 1;
                }
            }
            0xa6 => {
                // LDX - Load direct page into X
                let addr = self.load_direct(&mut cycles);
                if self.cpu().is_idx8() {
                    let x = self.read::<u8>(addr);
                    self.cpu_mut().update_nz8(x);
                    self.cpu_mut().regs.set_x8(x);
                } else {
                    let x = self.read::<u16>(addr);
                    self.cpu_mut().update_nz16(x);
                    self.cpu_mut().regs.x = x;
                    cycles += 1;
                }
            }
            0xa7 => {
                // LDA - Load direct page indirect long to A
                let addr = self.load_dp_indirect_long(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read(addr);
                    self.cpu_mut().regs.set_a8(val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    let val = self.read(addr);
                    self.cpu_mut().regs.a = val;
                    self.cpu_mut().update_nz16(val);
                    cycles += 1;
                }
            }
            0xa8 => {
                // TAY - Transfer A to Y
                if self.cpu().is_idx8() || self.cpu().regs.is_emulation {
                    let y = self.cpu().regs.a8();
                    self.cpu_mut().regs.set_y8(y);
                    self.cpu_mut().update_nz8(y);
                } else {
                    self.cpu_mut().regs.y = self.cpu().regs.a;
                    let y = self.cpu().regs.y;
                    self.cpu_mut().update_nz16(y);
                }
            }
            0xa9 => {
                // LDA - Load immediate value to A
                if self.cpu().is_reg8() {
                    let val = self.load::<u8>();
                    self.cpu_mut().update_nz8(val);
                    self.cpu_mut().regs.set_a8(val)
                } else {
                    let val = self.load::<u16>();
                    self.cpu_mut().update_nz16(val);
                    self.cpu_mut().regs.a = val;
                    cycles += 1;
                }
            }
            0xaa => {
                // TAX - Transfer A to X
                if self.cpu().is_idx8() || self.cpu().regs.is_emulation {
                    let x = self.cpu().regs.a8();
                    self.cpu_mut().regs.set_x8(x);
                    self.cpu_mut().update_nz8(x);
                } else {
                    self.cpu_mut().regs.x = self.cpu().regs.a;
                    let x = self.cpu().regs.x;
                    self.cpu_mut().update_nz16(x);
                }
            }
            0xab => {
                // PLB - Pull Data Bank
                let db = self.pull();
                self.cpu_mut().regs.db = db;
                self.cpu_mut().update_nz8(db)
            }
            0xac => {
                // LDY - Load absolute into Y
                let addr = self.load();
                let addr = self.cpu().get_data_addr(addr);
                if self.cpu().is_idx8() {
                    let y = self.read::<u8>(addr);
                    self.cpu_mut().update_nz8(y);
                    self.cpu_mut().regs.set_y8(y);
                } else {
                    let y = self.read::<u16>(addr);
                    self.cpu_mut().update_nz16(y);
                    self.cpu_mut().regs.y = y;
                    cycles += 1;
                }
            }
            0xad => {
                // LDA - Load absolute to A
                let addr = self.load();
                let addr = self.cpu().get_data_addr(addr);
                if self.cpu().is_reg8() {
                    let val = self.read(addr);
                    self.cpu_mut().regs.set_a8(val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    let val = self.read(addr);
                    self.cpu_mut().regs.a = val;
                    self.cpu_mut().update_nz16(val);
                    cycles += 1;
                }
            }
            0xae => {
                // LDX - Load absolute into X
                let addr = self.load::<u16>();
                let addr = self.cpu().get_data_addr(addr);
                if self.cpu().is_idx8() {
                    let x = self.read::<u8>(addr);
                    self.cpu_mut().update_nz8(x);
                    self.cpu_mut().regs.set_x8(x);
                } else {
                    let x = self.read::<u16>(addr);
                    self.cpu_mut().update_nz16(x);
                    self.cpu_mut().regs.x = x;
                    cycles += 1;
                }
            }
            0xaf => {
                // LDA - Load Absolute Long to A
                let addr = self.load::<Addr24>();
                if self.cpu().is_reg8() {
                    let val = self.read(addr);
                    self.cpu_mut().regs.set_a8(val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    let val = self.read(addr);
                    self.cpu_mut().regs.a = val;
                    self.cpu_mut().update_nz16(val);
                    cycles += 1;
                }
            }
            0xb0 => {
                // BCS/BGE - Branch if carry set
                self.branch_near(self.cpu().regs.status.has(Status::CARRY), &mut cycles)
            }
            0xb1 => {
                // LDA - Load DP Indirect Indexed, Y value to A
                let addr = self.load_indirect_indexed_y::<true>(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr);
                    self.cpu_mut().update_nz8(val);
                    self.cpu_mut().regs.set_a8(val)
                } else {
                    let val = self.read::<u16>(addr);
                    self.cpu_mut().update_nz16(val);
                    self.cpu_mut().regs.a = val;
                    cycles += 1;
                }
            }
            0xb2 => {
                // LDA - Load DP indirect value to A
                let addr = self.load_dp_indirect(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr);
                    self.cpu_mut().update_nz8(val);
                    self.cpu_mut().regs.set_a8(val)
                } else {
                    let val = self.read::<u16>(addr);
                    self.cpu_mut().update_nz16(val);
                    self.cpu_mut().regs.a = val;
                    cycles += 1;
                }
            }
            0xb3 => {
                // LDA - Load SR Indirect Indexed,Y into A
                let addr = self.load_sr_indirect_indexed_y();
                if self.cpu().is_reg8() {
                    let val = self.read(addr);
                    self.cpu_mut().regs.set_a8(val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    let val = self.read(addr);
                    self.cpu_mut().regs.a = val;
                    self.cpu_mut().update_nz16(val);
                    cycles += 1;
                }
            }
            0xb4 => {
                // LDY - Load DP Indexed, X into Y
                let addr = self.load_dp_indexed_x(&mut cycles);
                if self.cpu().is_idx8() {
                    let y = self.read::<u8>(addr);
                    self.cpu_mut().update_nz8(y);
                    self.cpu_mut().regs.set_y8(y);
                } else {
                    let y = self.read::<u16>(addr);
                    self.cpu_mut().update_nz16(y);
                    self.cpu_mut().regs.y = y;
                    cycles += 1;
                }
            }
            0xb5 => {
                // LDA - Load DP Indexed, X into A
                let addr = self.load_dp_indexed_x(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr);
                    self.cpu_mut().update_nz8(val);
                    self.cpu_mut().regs.set_a8(val)
                } else {
                    let val = self.read::<u16>(addr);
                    self.cpu_mut().update_nz16(val);
                    self.cpu_mut().regs.a = val;
                    cycles += 1;
                }
            }
            0xb6 => {
                // LDX - Load DP Indexed, Y into X
                let addr = self.load_dp_indexed_y(&mut cycles);
                if self.cpu().is_idx8() {
                    let x = self.read::<u8>(addr);
                    self.cpu_mut().update_nz8(x);
                    self.cpu_mut().regs.set_x8(x);
                } else {
                    let x = self.read::<u16>(addr);
                    self.cpu_mut().update_nz16(x);
                    self.cpu_mut().regs.x = x;
                    cycles += 1;
                }
            }
            0xb7 => {
                // LDA - Load indirect long indexed Y value to A
                let addr = self.load_indirect_long_indexed_y(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr);
                    self.cpu_mut().update_nz8(val);
                    self.cpu_mut().regs.set_a8(val)
                } else {
                    let val = self.read::<u16>(addr);
                    self.cpu_mut().update_nz16(val);
                    self.cpu_mut().regs.a = val;
                    cycles += 1;
                }
            }
            0xb8 => {
                // CLV - Clear Overflow flag
                self.cpu_mut().regs.status &= !Status::OVERFLOW
            }
            0xb9 => {
                // LDA - Load absolute indexed Y value to A
                let addr = self.load_indexed_y::<true>(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read(addr);
                    self.cpu_mut().regs.set_a8(val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    self.cpu_mut().regs.a = self.read(addr);
                    let a = self.cpu().regs.a;
                    self.cpu_mut().update_nz16(a);
                    cycles += 1
                }
            }
            0xba => {
                // TSX - Transfer SP to X
                if self.cpu().is_idx8() {
                    let val = (self.cpu().regs.sp & 0xff) as u8;
                    self.cpu_mut().regs.set_x8(val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    self.cpu_mut().regs.x = self.cpu().regs.sp;
                    let x = self.cpu().regs.x;
                    self.cpu_mut().update_nz16(x);
                }
            }
            0xbb => {
                // TYX - Transfer Y to X
                if self.cpu().is_idx8() {
                    let y = self.cpu().regs.y8();
                    self.cpu_mut().regs.set_x8(y);
                    self.cpu_mut().update_nz8(y);
                } else {
                    self.cpu_mut().regs.x = self.cpu().regs.y;
                    let y = self.cpu().regs.y;
                    self.cpu_mut().update_nz16(y);
                }
            }
            0xbc => {
                // LDY - Load indexed, X into Y
                let addr = self.load_indexed_x::<true>(&mut cycles);
                if self.cpu().is_idx8() {
                    let y = self.read::<u8>(addr);
                    self.cpu_mut().update_nz8(y);
                    self.cpu_mut().regs.set_y8(y);
                } else {
                    let y = self.read::<u16>(addr);
                    self.cpu_mut().update_nz16(y);
                    self.cpu_mut().regs.y = y;
                    cycles += 1;
                }
            }
            0xbd => {
                // LDA - Load absolute indexed X value to A
                let addr = self.load_indexed_x::<true>(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read(addr);
                    self.cpu_mut().regs.set_a8(val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    self.cpu_mut().regs.a = self.read(addr);
                    let a = self.cpu().regs.a;
                    self.cpu_mut().update_nz16(a);
                    cycles += 1
                }
            }
            0xbe => {
                // LDX - Load absolute indexed, Y into X
                let addr = self.load_indexed_y::<true>(&mut cycles);
                if self.cpu().is_idx8() {
                    let x = self.read::<u8>(addr);
                    self.cpu_mut().update_nz8(x);
                    self.cpu_mut().regs.set_x8(x);
                } else {
                    let x = self.read::<u16>(addr);
                    self.cpu_mut().update_nz16(x);
                    self.cpu_mut().regs.x = x;
                    cycles += 1;
                }
            }
            0xbf => {
                // LDA - Load absolute long indexed X value to A
                let addr = self.load_long_indexed_x();
                if self.cpu().is_reg8() {
                    let val = self.read(addr);
                    self.cpu_mut().regs.set_a8(val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    self.cpu_mut().regs.a = self.read(addr);
                    let a = self.cpu().regs.a;
                    self.cpu_mut().update_nz16(a);
                    cycles += 1
                }
            }
            0xc0 => {
                // CPY - Compare Y with immediate value
                if self.cpu().is_idx8() {
                    let val = self.load::<u8>();
                    self.compare8(self.cpu().regs.y8(), val);
                } else {
                    let val = self.load::<u16>();
                    self.compare16(self.cpu().regs.y, val);
                    cycles += 1
                }
            }
            0xc1 => {
                // CMP - Compare A with DP Indexed Indirect, X
                let addr = self.load_dp_indexed_indirect_x(&mut cycles);
                compare_memory!(CMP: self, addr, &mut cycles)
            }
            0xc2 => {
                // REP - Reset specified bits in the Status Register
                let mask = Status(!self.load::<u8>());
                self.cpu_mut().regs.status &= mask;
                self.cpu_mut().update_status();
            }
            0xc3 => {
                // CMP - Compare A with Stack Relative
                let addr = self.load_stack_relative();
                compare_memory!(CMP: self, addr, &mut cycles)
            }
            0xc4 => {
                // CPY - Compare Y with direct page
                let addr = self.load_direct(&mut cycles);
                compare_memory!(CPY: self, addr, &mut cycles)
            }
            0xc5 => {
                // CMP - Compare A with Absolute Indexed, Y
                let addr = self.load_direct(&mut cycles);
                compare_memory!(CMP: self, addr, &mut cycles)
            }
            0xc6 => {
                // DEC - Decrement DP
                let addr = self.load_direct(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr).wrapping_sub(1);
                    self.write(addr, val);
                    self.cpu_mut().update_nz8(val)
                } else {
                    let val = self.read::<u16>(addr).wrapping_sub(1);
                    self.write(addr, val);
                    self.cpu_mut().update_nz16(val);
                    cycles += 2
                }
            }
            0xc7 => {
                // CMP - Compare A with DP Indirect Long
                let addr = self.load_dp_indirect_long(&mut cycles);
                compare_memory!(CMP: self, addr, &mut cycles)
            }
            0xc8 => {
                // INY - Increment Y
                if self.cpu().is_idx8() {
                    let y = self.cpu().regs.y8().wrapping_add(1);
                    self.cpu_mut().regs.set_y8(y);
                    self.cpu_mut().update_nz8(y);
                } else {
                    self.cpu_mut().regs.y = self.cpu().regs.y.wrapping_add(1);
                    let y = self.cpu().regs.y;
                    self.cpu_mut().update_nz16(y);
                }
            }
            0xc9 => {
                // CMP - Compare A with immediate value
                if self.cpu().is_reg8() {
                    let val = self.load::<u8>();
                    self.compare8(self.cpu().regs.a8(), val);
                } else {
                    let val = self.load::<u16>();
                    self.compare16(self.cpu().regs.a, val);
                    cycles += 1
                }
            }
            0xca => {
                // DEX - Decrement X
                if self.cpu().is_idx8() {
                    let x = self.cpu().regs.x8().wrapping_sub(1);
                    self.cpu_mut().regs.set_x8(x);
                    self.cpu_mut().update_nz8(x);
                } else {
                    self.cpu_mut().regs.x = self.cpu().regs.x.wrapping_sub(1);
                    let x = self.cpu().regs.x;
                    self.cpu_mut().update_nz16(x);
                }
            }
            0xcb => {
                // WAI - Wait until interrupt
                self.cpu_mut().wait_mode = true;
            }
            0xcc => {
                // CPY - Compare Y with absolute value
                let addr = self.load();
                let addr = self.cpu().get_data_addr(addr);
                compare_memory!(CPY: self, addr, &mut cycles)
            }
            0xcd => {
                // CMP - Compare A with absolute value
                let addr = self.load::<u16>();
                let addr = self.cpu().get_data_addr(addr);
                compare_memory!(CMP: self, addr, &mut cycles)
            }
            0xce => {
                // DEC - Decrement absolute
                let addr = self.load();
                let addr = self.cpu().get_data_addr(addr);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr).wrapping_sub(1);
                    self.write(addr, val);
                    self.cpu_mut().update_nz8(val)
                } else {
                    let val = self.read::<u16>(addr).wrapping_sub(1);
                    self.write(addr, val);
                    self.cpu_mut().update_nz16(val);
                    cycles += 2
                }
            }
            0xcf => {
                // CMP - Compare A with Absolute Long
                // this will also work with decimal mode (TODO: check this fact)
                let addr = self.load::<Addr24>();
                compare_memory!(CMP: self, addr, &mut cycles)
            }
            0xd0 => {
                // BNE - Branch if Zero Flag Clear
                self.branch_near(!self.cpu().regs.status.has(Status::ZERO), &mut cycles)
            }
            0xd1 => {
                // CMP - Compare A with DP Indirect Indexed, Y
                // this will also work with decimal mode (TODO: check this fact)
                let addr = self.load_indirect_indexed_y::<true>(&mut cycles);
                compare_memory!(CMP: self, addr, &mut cycles)
            }
            0xd2 => {
                // CMP - Compare A with DP Indirect
                let addr = self.load_dp_indirect(&mut cycles);
                compare_memory!(CMP: self, addr, &mut cycles)
            }
            0xd3 => {
                // CMP - Compare A with SR Indirect Indexed, Y
                let addr = self.load_sr_indirect_indexed_y();
                compare_memory!(CMP: self, addr, &mut cycles)
            }
            0xd4 => {
                // PEI - Push 16-bit value from DP
                let addr = self.load_direct(&mut cycles);
                let val = self.read::<u16>(addr);
                self.push(val);
            }
            0xd5 => {
                // CMP - Compare A with DP Indexed, X
                let addr = self.load_dp_indexed_x(&mut cycles);
                compare_memory!(CMP: self, addr, &mut cycles)
            }
            0xd6 => {
                // DEC - Decrement DP Indexed, X
                let addr = self.load_dp_indexed_x(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr).wrapping_sub(1);
                    self.write(addr, val);
                    self.cpu_mut().update_nz8(val)
                } else {
                    let val = self.read::<u16>(addr).wrapping_sub(1);
                    self.write(addr, val);
                    self.cpu_mut().update_nz16(val);
                    cycles += 2
                }
            }
            0xd7 => {
                // CMP - Compare A with DP Indirect Long Indexed, Y
                let addr = self.load_indirect_long_indexed_y(&mut cycles);
                compare_memory!(CMP: self, addr, &mut cycles)
            }
            0xd8 => {
                // CLD - Clear Decimal Flag
                self.cpu_mut().regs.status &= !Status::DECIMAL
            }
            0xd9 => {
                // CMP - Compare A with Absolute Indexed, Y
                let addr = self.load_indexed_y::<true>(&mut cycles);
                compare_memory!(CMP: self, addr, &mut cycles)
            }
            0xda => {
                // PHX - Push X
                if self.cpu().is_idx8() {
                    self.push(self.cpu().regs.x8())
                } else {
                    self.push(self.cpu().regs.x);
                    cycles += 1
                }
            }
            0xdb => {
                // STP - Stop Processor
                self.cpu_mut().active = false
            }
            0xdc => {
                // JMP/JML - Jump absolute indirect long
                let addr = self.load();
                let addr = self.cpu().get_data_addr(addr);
                self.cpu_mut().regs.pc = self.read::<Addr24>(addr);
            }
            0xdd => {
                // CMP - Compare A with Absolute Indexed, X
                let addr = self.load_indexed_x::<true>(&mut cycles);
                compare_memory!(CMP: self, addr, &mut cycles)
            }
            0xde => {
                // DEC - Decrement Absolute Indexed, X
                let addr = self.load_indexed_x::<false>(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr).wrapping_sub(1);
                    self.write(addr, val);
                    self.cpu_mut().update_nz8(val)
                } else {
                    let val = self.read::<u16>(addr).wrapping_sub(1);
                    self.write(addr, val);
                    self.cpu_mut().update_nz16(val);
                    cycles += 2
                }
            }
            0xdf => {
                // CMP - Compare A with Absolute Long Indexed, X
                let addr = self.load_long_indexed_x();
                compare_memory!(CMP: self, addr, &mut cycles)
            }
            0xe0 => {
                // CPX - Compare X with immediate value
                if self.cpu().is_idx8() {
                    let val = self.load::<u8>();
                    self.compare8(self.cpu().regs.x8(), val);
                } else {
                    let val = self.load::<u16>();
                    self.compare16(self.cpu().regs.x, val);
                    cycles += 1
                }
            }
            0xe1 => {
                // SBC - Subtract DP Indexed Indirect, X with carry
                let addr = self.load_dp_indexed_indirect_x(&mut cycles);
                self.sub_carry_memory(addr, &mut cycles)
            }
            0xe2 => {
                // SEP - Set specified bits in the Status Register
                let mask = Status(self.load::<u8>());
                self.cpu_mut().regs.status |= mask;
                self.cpu_mut().update_status();
            }
            0xe3 => {
                // SBC - Subtract Stack Relative with carry
                let addr = self.load_stack_relative();
                self.sub_carry_memory(addr, &mut cycles)
            }
            0xe4 => {
                // CPX - Compare X with Direct Page
                let addr = self.load_direct(&mut cycles);
                compare_memory!(CPX: self, addr, &mut cycles)
            }
            0xe5 => {
                // SBC - Subtract Direct Page with carry
                let addr = self.load_direct(&mut cycles);
                self.sub_carry_memory(addr, &mut cycles)
            }
            0xe6 => {
                // INC - Increment direct page
                let addr = self.load_direct(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr).wrapping_add(1);
                    self.write::<u8>(addr, val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    let val = self.read::<u16>(addr).wrapping_add(1);
                    self.write::<u16>(addr, val);
                    self.cpu_mut().update_nz16(val);
                    cycles += 2
                }
            }
            0xe7 => {
                // SBC - Subtract DP Indirect Long with carry
                let addr = self.load_dp_indirect_long(&mut cycles);
                self.sub_carry_memory(addr, &mut cycles)
            }
            0xe8 => {
                // INX - Increment X
                if self.cpu().is_idx8() {
                    let x = self.cpu().regs.x8().wrapping_add(1);
                    self.cpu_mut().regs.set_x8(x);
                    self.cpu_mut().update_nz8(x);
                } else {
                    self.cpu_mut().regs.x = self.cpu().regs.x.wrapping_add(1);
                    let x = self.cpu().regs.x;
                    self.cpu_mut().update_nz16(x);
                }
            }
            0xe9 => {
                // SBC - Subtract with carry
                if self.cpu().is_reg8() {
                    let op1 = self.load::<u8>();
                    self.sub_carry8(op1);
                } else {
                    let op1 = self.load::<u16>();
                    self.sub_carry16(op1);
                    cycles += 1;
                }
            }
            0xea => (), // NOP
            0xeb => {
                // XBA - Swap the A Register
                self.cpu_mut().regs.a = self.cpu().regs.a.swap_bytes();
                let a = self.cpu().regs.a8();
                self.cpu_mut().update_nz8(a)
            }
            0xec => {
                // CPX - Compare X with absolute value
                let addr = self.load();
                let addr = self.cpu().get_data_addr(addr);
                compare_memory!(CPX: self, addr, &mut cycles)
            }
            0xed => {
                // SBC - Subtract absolute with carry
                let addr = self.load::<u16>();
                let addr = self.cpu().get_data_addr(addr);
                self.sub_carry_memory(addr, &mut cycles)
            }
            0xee => {
                // INC - Increment absolute
                let addr = self.load();
                let addr = self.cpu().get_data_addr(addr);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr).wrapping_add(1);
                    self.write::<u8>(addr, val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    let val = self.read::<u16>(addr).wrapping_add(1);
                    self.write::<u16>(addr, val);
                    self.cpu_mut().update_nz16(val);
                    cycles += 2
                }
            }
            0xef => {
                // SBC - Subtract Absolute Long with carry
                let addr = self.load::<Addr24>();
                self.sub_carry_memory(addr, &mut cycles)
            }
            0xf0 => {
                // BEQ - Branch if ZERO is set
                self.branch_near(self.cpu().regs.status.has(Status::ZERO), &mut cycles)
            }
            0xf1 => {
                // SBC - Subtract DP Indirect Indexed, Y with carry
                let addr = self.load_indirect_indexed_y::<true>(&mut cycles);
                self.sub_carry_memory(addr, &mut cycles)
            }
            0xf2 => {
                // SBC - Subtract DP Indirect with carry
                let addr = self.load_dp_indirect(&mut cycles);
                self.sub_carry_memory(addr, &mut cycles)
            }
            0xf3 => {
                // SBC - Subtract SR Indirect Indexed, Y with carry
                let addr = self.load_sr_indirect_indexed_y();
                self.sub_carry_memory(addr, &mut cycles)
            }
            0xf4 => {
                // PEA - Push absolute value
                let addr = self.load::<u16>();
                self.push(addr)
            }
            0xf5 => {
                // SBC - Subtract DP Indexed, X with carry
                let addr = self.load_dp_indexed_x(&mut cycles);
                self.sub_carry_memory(addr, &mut cycles)
            }
            0xf6 => {
                // INC - Increment DP Indexed, X
                let addr = self.load_dp_indexed_x(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr).wrapping_add(1);
                    self.write::<u8>(addr, val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    let val = self.read::<u16>(addr).wrapping_add(1);
                    self.write::<u16>(addr, val);
                    self.cpu_mut().update_nz16(val);
                    cycles += 2
                }
            }
            0xf7 => {
                // SBC - Subtract DP Indirect Long Indexed, Y with carry
                let addr = self.load_indirect_long_indexed_y(&mut cycles);
                self.sub_carry_memory(addr, &mut cycles)
            }
            0xf8 => {
                // SED - Set Decimal flag
                self.cpu_mut().regs.status |= Status::DECIMAL
            }
            0xf9 => {
                // SBC - Subtract Absolute Indexed, Y with carry
                let addr = self.load_indexed_y::<true>(&mut cycles);
                self.sub_carry_memory(addr, &mut cycles)
            }
            0xfa => {
                // PLX - Pull X
                if self.cpu().is_idx8() {
                    let x = self.pull();
                    self.cpu_mut().regs.set_x8(x);
                    self.cpu_mut().update_nz8(x);
                } else {
                    let x = self.pull();
                    self.cpu_mut().regs.x = x;
                    self.cpu_mut().update_nz16(x);
                    cycles += 1
                }
            }
            0xfb => {
                // XCE - Swap Carry and Emulation Flags
                let carry = self.cpu().regs.status.has(Status::CARRY);
                let is_emu = self.cpu().regs.is_emulation;
                self.cpu_mut().regs.status.set_if(Status::CARRY, is_emu);
                self.cpu_mut().set_emulation(carry);
            }
            0xfc => {
                // JSR - Jump to Subroutine
                let addr = self.load_indexed_indirect();
                self.push(start_addr.addr.wrapping_add(2));
                self.cpu_mut().regs.pc = addr;
            }
            0xfd => {
                // SBC - Subtract Absolute Indexed, X with carry
                let addr = self.load_indexed_x::<true>(&mut cycles);
                self.sub_carry_memory(addr, &mut cycles)
            }
            0xfe => {
                // INC - Increment Absolute Indexed, X
                let addr = self.load_indexed_x::<false>(&mut cycles);
                if self.cpu().is_reg8() {
                    let val = self.read::<u8>(addr).wrapping_add(1);
                    self.write::<u8>(addr, val);
                    self.cpu_mut().update_nz8(val);
                } else {
                    let val = self.read::<u16>(addr).wrapping_add(1);
                    self.write::<u16>(addr, val);
                    self.cpu_mut().update_nz16(val);
                    cycles += 2
                }
            }
            0xff => {
                // SBC - Subtract Absolute Long Indexed, X with carry
                let addr = self.load_long_indexed_x();
                self.sub_carry_memory(addr, &mut cycles)
            }
        };
        cycles
    }

    fn block_move<const DELTA: u16>(&mut self) {
        let [dst, src] = self.load::<u16>().to_bytes();
        self.cpu_mut().regs.db = dst;
        let src = Addr24::new(src, self.cpu().regs.x);
        let dst = Addr24::new(dst, self.cpu().regs.y);
        let val = self.read::<u8>(src);
        self.write::<u8>(dst, val);
        if self.cpu().is_idx8() {
            let x = self.cpu().regs.x8().wrapping_add((DELTA & 0xff) as u8);
            self.cpu_mut().regs.set_x8(x);
            let y = self.cpu().regs.y8().wrapping_add((DELTA & 0xff) as u8);
            self.cpu_mut().regs.set_y8(y);
        } else {
            self.cpu_mut().regs.x = self.cpu().regs.x.wrapping_add(DELTA);
            self.cpu_mut().regs.y = self.cpu().regs.y.wrapping_add(DELTA);
        }
        self.cpu_mut().regs.a = self.cpu().regs.a.wrapping_sub(1);
        if self.cpu().regs.a != u16::MAX {
            self.cpu_mut().regs.pc.addr = self.cpu().regs.pc.addr.wrapping_sub(3);
        }
    }

    fn rotate_left(&mut self, addr: Addr24, cycles: &mut Cycles) {
        if self.cpu().is_reg8() {
            let val = self.read::<u8>(addr);
            let res = self.cpu().regs.status.has(Status::CARRY) as u8 | (val << 1);
            self.cpu_mut()
                .regs
                .status
                .set_if(Status::CARRY, val & 0x80 > 0);
            self.cpu_mut().update_nz8(res);
            self.write(addr, res);
        } else {
            let val = self.read::<u16>(addr);
            let res = self.cpu().regs.status.has(Status::CARRY) as u16 | (val << 1);
            self.cpu_mut()
                .regs
                .status
                .set_if(Status::CARRY, val & 0x8000 > 0);
            self.cpu_mut().update_nz16(res);
            self.write(addr, res);
            *cycles += 2
        }
    }

    fn add_carry_memory(&mut self, addr: Addr24, cycles: &mut Cycles) {
        if self.cpu().is_reg8() {
            let op1 = self.read::<u8>(addr);
            self.add_carry8(op1);
        } else {
            let op1 = self.read::<u16>(addr);
            self.add_carry16(op1);
            *cycles += 1;
        }
    }

    fn sub_carry_memory(&mut self, addr: Addr24, cycles: &mut Cycles) {
        if self.cpu().is_reg8() {
            let op1 = self.read::<u8>(addr);
            self.sub_carry8(op1);
        } else {
            let op1 = self.read::<u16>(addr);
            self.sub_carry16(op1);
            *cycles += 1;
        }
    }

    fn test_bit(&mut self, addr: Addr24, cycles: &mut Cycles) {
        if self.cpu().is_reg8() {
            let a = self.cpu().regs.a8();
            let val = self.read::<u8>(addr);
            self.cpu_mut().regs.status.0 = (self.cpu().regs.status.0 & 0x3f) | (val & 0xc0);
            self.cpu_mut()
                .regs
                .status
                .set_if(Status::ZERO, a & val == 0)
        } else {
            let a = self.cpu().regs.a;
            let val = self.read::<u16>(addr);
            self.cpu_mut().regs.status.0 =
                (self.cpu().regs.status.0 & 0x3f) | ((val >> 8) as u8 & 0xc0);
            self.cpu_mut()
                .regs
                .status
                .set_if(Status::ZERO, a & val == 0);
            *cycles += 1
        }
    }

    fn generic_add_carry8<const GT1: u8, const GT2: u16>(
        &mut self,
        op1: u8,
        fu8: fn(u8, u8) -> u8,
        gt8: fn(&u8, &u8) -> bool,
        fu16: fn(u16, u16) -> u16,
        gt16: fn(&u16, &u16) -> bool,
    ) {
        let op2 = self.cpu().regs.a8();
        if self.cpu().regs.status.has(Status::DECIMAL) {
            let res = (op1 & 0xf)
                .wrapping_add(op2 & 0xf)
                .wrapping_add(self.cpu().regs.status.has(Status::CARRY) as _);
            let res = if gt8(&res, &GT1) { fu8(res, 6) } else { res };
            let carry = (res > 0xf) as u16;
            let res = u16::from(op1 & 0xf0)
                .wrapping_add((op2 & 0xf0).into())
                .wrapping_add(carry << 4)
                .wrapping_add((res & 0xf).into());
            self.cpu_mut().regs.status.set_if(
                Status::OVERFLOW,
                !(u16::from(op1) ^ u16::from(op2)) & (u16::from(op2) ^ res) & 0x80 > 0,
            );
            let res = if gt16(&res, &GT2) {
                fu16(res, 0x60)
            } else {
                res
            };
            self.cpu_mut().regs.status.set_if(Status::CARRY, res > 0xff);
            let res = (res & 0xff) as u8;
            self.cpu_mut().update_nz8(res);
            self.cpu_mut().regs.set_a8(res);
        } else {
            let (new, nc) = op1.overflowing_add(op2);
            let (new, nc2) = new.overflowing_add(self.cpu().regs.status.has(Status::CARRY) as _);
            let nc = nc ^ nc2;
            self.cpu_mut().regs.status.set_if(Status::CARRY, nc);
            let op1v = op1 & 128;
            let v = op1v == (op2 & 128) && op1v != (new & 128);
            self.cpu_mut().regs.status.set_if(Status::OVERFLOW, v);
            self.cpu_mut().update_nz8(new);
            self.cpu_mut().regs.set_a8(new);
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
        let op2 = self.cpu().regs.a;
        if self.cpu().regs.status.has(Status::DECIMAL) {
            let res = (op1 & 0xf)
                .wrapping_add(op2 & 0xf)
                .wrapping_add(self.cpu().regs.status.has(Status::CARRY) as _);
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
            self.cpu_mut().regs.status.set_if(
                Status::OVERFLOW,
                !(u32::from(op1) ^ u32::from(op2)) & (u32::from(op2) ^ res) & 0x8000 > 0,
            );
            let res = if gt32(&res, &GT4) {
                fu32(res, 0x6000)
            } else {
                res
            };
            self.cpu_mut()
                .regs
                .status
                .set_if(Status::CARRY, res > 0xffff);
            let res = (res & 0xffff) as u16;
            self.cpu_mut().update_nz16(res);
            self.cpu_mut().regs.a = res
        } else {
            let (new, nc) = op1.overflowing_add(op2);
            let (new, nc2) = new.overflowing_add(self.cpu().regs.status.has(Status::CARRY) as _);
            let nc = nc ^ nc2;
            self.cpu_mut().regs.status.set_if(Status::CARRY, nc);
            let op1v = op1 & 0x8000;
            let v = op1v == (op2 & 0x8000) && op1v != (new & 0x8000);
            self.cpu_mut().regs.status.set_if(Status::OVERFLOW, v);
            self.cpu_mut().update_nz16(new);
            self.cpu_mut().regs.a = new;
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
                self.cpu().regs.pc.addr.wrapping_sub(rel.into())
            } else {
                self.cpu().regs.pc.addr.wrapping_add(rel.into())
            };
            let old = core::mem::replace(&mut self.cpu_mut().regs.pc.addr, new);
            if self.cpu().regs.is_emulation && old & 0xff00 != new & 0xff00 {
                *cycles += 1
            }
        }
    }

    pub fn store_zero(&mut self, addr: Addr24, cycles: &mut Cycles) {
        if self.cpu().is_reg8() {
            self.write(addr, 0u8);
        } else {
            self.write(addr, 0u16);
            *cycles += 1;
        }
    }

    pub fn exclusive_or(&mut self, addr: Addr24, cycles: &mut Cycles) {
        if self.cpu().is_reg8() {
            let val = self.read::<u8>(addr) ^ self.cpu().regs.a8();
            self.cpu_mut().regs.set_a8(val);
            self.cpu_mut().update_nz8(val);
        } else {
            let val = self.read::<u16>(addr) ^ self.cpu().regs.a;
            self.cpu_mut().regs.a = val;
            self.cpu_mut().update_nz16(val);
            *cycles += 1
        }
    }

    pub fn and(&mut self, addr: Addr24, cycles: &mut Cycles) {
        if self.cpu().is_reg8() {
            let val = self.read::<u8>(addr) & self.cpu().regs.a8();
            self.cpu_mut().regs.set_a8(val);
            self.cpu_mut().update_nz8(val);
        } else {
            let val = self.read::<u16>(addr) & self.cpu().regs.a;
            self.cpu_mut().regs.a = val;
            self.cpu_mut().update_nz16(val);
            *cycles += 1
        }
    }

    pub fn ora(&mut self, addr: Addr24, cycles: &mut Cycles) {
        if self.cpu().is_reg8() {
            let val = self.read::<u8>(addr) | self.cpu().regs.a8();
            self.cpu_mut().regs.set_a8(val);
            self.cpu_mut().update_nz8(val);
        } else {
            let val = self.read::<u16>(addr) | self.cpu().regs.a;
            self.cpu_mut().regs.a = val;
            self.cpu_mut().update_nz16(val);
            *cycles += 1
        }
    }

    pub fn compare8(&mut self, a: u8, b: u8) {
        let res = a as u16 + (!b) as u16 + 1;
        self.cpu_mut().regs.status.set_if(Status::CARRY, res > 0xff);
        self.cpu_mut().update_nz8((res & 0xff) as u8);
    }

    pub fn compare16(&mut self, a: u16, b: u16) {
        let res = a as u32 + (!b) as u32 + 1;
        self.cpu_mut()
            .regs
            .status
            .set_if(Status::CARRY, res > 0xffff);
        self.cpu_mut().update_nz16((res & 0xffff) as u16);
    }

    pub fn dispatch_instruction(&mut self) -> Cycles {
        let pc = self.cpu().regs.pc;
        let op = self.load::<u8>();
        self.dispatch_instruction_with(pc, op)
    }

    pub fn nmi(&mut self) -> u32 {
        self.cpu_mut().in_nmi = true;
        self.interrupt(if self.cpu().regs.is_emulation {
            0xfffa
        } else {
            0xffea
        })
    }

    pub fn irq(&mut self) -> u32 {
        self.cpu_mut().irq_bit = 0x80;
        self.interrupt(if self.cpu().regs.is_emulation {
            0xfffe
        } else {
            0xffee
        })
    }

    pub fn interrupt(&mut self, vector: u16) -> u32 {
        if self.cpu().regs.is_emulation {
            self.push(self.cpu().regs.pc.addr)
        } else {
            self.push(self.cpu().regs.pc)
        }
        self.push(self.cpu().regs.status.0);
        self.cpu_mut().regs.status |= Status::IRQ_DISABLE;
        self.cpu_mut().regs.status &= !Status::DECIMAL;
        let addr = self.read(Addr24::new(0, vector));
        self.cpu_mut().regs.pc = Addr24::new(0, addr);
        48
    }
}
