//! SA-1 Coprocessor handling types
//!
//! # Literature
//!
//! - <https://wiki.superfamicom.org/sa-1>
//! - <https://wiki.superfamicom.org/sa-1-registers>
//! - <https://wiki.superfamicom.org/uploads/assembly-programming-manual-for-w65c816.pdf>
//! - <https://problemkaputt.de/fullsnes.htm>

use crate::{
    cpu::Cpu,
    device::{Addr24, Data, Device},
    instr::{AccessType, DeviceAccess},
};
use core::mem::replace;
use save_state_macro::*;

const IRAM_SIZE: usize = 0x800;
const BWRAM_SIZE: usize = 0x40000;

#[derive(Debug, Default, Clone, Copy, InSaveState)]
struct Block {
    hirom_bank: u8,
    lorom_bank: u8,
}

impl Block {
    pub const fn new(id: u8, val: u8) -> Self {
        let bank = (val & 7) << 4;
        let lorom = val & 0x80 > 0;
        Self {
            hirom_bank: bank,
            lorom_bank: if lorom { bank } else { id },
        }
    }

    pub fn lorom(&self, addr: Addr24) -> u32 {
        (u32::from(self.lorom_bank) << 16)
            | (u32::from(addr.bank & 0x1f) << 15)
            | u32::from(addr.addr & 0x7fff)
    }

    pub fn hirom(&self, addr: Addr24) -> u32 {
        (u32::from((addr.bank & 0xf) | self.hirom_bank) << 16) | u32::from(addr.addr)
    }
}

#[derive(Debug, DefaultByNew, Clone, InSaveState)]
pub struct Sa1 {
    iram: [u8; IRAM_SIZE],
    bwram: [u8; BWRAM_SIZE],
    blocks: [Block; 4],
    cpu: Cpu,
    ahead_cycles: i32,
    shall_nmi: bool,
    shall_irq: bool,
    reset_vector_override: u16,
    irq_enable: bool,
    char_dma_irq_enable: bool,
    snes_control_flags: u8,
    control_flags: u8,
    bwram_map: [u8; 2],

    // SA-1-side interrupt flags
    // 0x10: NMI from SNES
    // 0x20: DMA IRQ
    // 0x40: Timer IRQ
    // 0x80: IRQ from SNES
    sa1_interrupt_enable: u8,
    sa1_interrupt_acknowledge: u8,
    sa1_interrupt_trigger: u8,
}

impl Sa1 {
    pub const fn new() -> Self {
        Self {
            iram: [0; IRAM_SIZE],
            bwram: [0; BWRAM_SIZE],
            blocks: [
                Block::new(0, 0), // Set Super MMC Bank C
                Block::new(1, 1), // Set Super MMC Bank D
                Block::new(2, 2), // Set Super MMC Bank E
                Block::new(3, 3), // Set Super MMC Bank F
            ],
            cpu: Cpu::new(),
            ahead_cycles: 0,
            shall_nmi: false,
            shall_irq: false,
            reset_vector_override: 0,
            irq_enable: false,
            char_dma_irq_enable: false,
            snes_control_flags: 0,
            control_flags: 0x20,
            bwram_map: [0; 2],

            sa1_interrupt_enable: 0,
            sa1_interrupt_acknowledge: 0,
            sa1_interrupt_trigger: 0,
        }
    }

    pub fn reset(&mut self) {
        // TODO: correctly implement resetting
        *self = Self::new()
    }

    pub fn cpu_mut(&mut self) -> &mut Cpu {
        &mut self.cpu
    }

    pub fn lorom_addr(&self, addr: Addr24) -> u32 {
        match addr.bank {
            0x00..=0x1f => self.blocks[0].lorom(addr),
            0x20..=0x3f => self.blocks[1].lorom(addr),
            0x80..=0x9f => self.blocks[2].lorom(addr),
            0xa0..=0xbf => self.blocks[3].lorom(addr),
            _ => unreachable!(),
        }
    }

    pub fn hirom_addr(&self, addr: Addr24) -> u32 {
        match addr.bank {
            0xc0..=0xcf => self.blocks[0].hirom(addr),
            0xd0..=0xdf => self.blocks[1].hirom(addr),
            0xe0..=0xef => self.blocks[2].hirom(addr),
            0xf0..=0xff => self.blocks[3].hirom(addr),
            _ => unreachable!(),
        }
    }

    pub fn read_io<const INTERNAL: bool>(&mut self, id: u16) -> u8 {
        const SA1: bool = true;
        const SNES: bool = false;
        match (id, INTERNAL) {
            (0x2300, SNES) => {
                // SCNT - SNES Control flags
                // TODO: IRQ from Character Conversion DMA
                // TODO: IRQ from SA-1 to SNES
                self.snes_control_flags & 0x5f
            }
            _ => todo!("read SA-1 io port {id:04x}"),
        }
    }

    pub fn write_io<const INTERNAL: bool>(&mut self, id: u16, val: u8) {
        const SA1: bool = true;
        const SNES: bool = false;
        match (id, INTERNAL) {
            (0x2200, SNES) => {
                // CCNT - Control SA-1 from SNES
                if (replace(&mut self.control_flags, val) & !val) & 0x20 > 0 {
                    self.cpu.regs.pc = Addr24::new(0, self.reset_vector_override)
                }
                let en = val & 0x90;
                self.sa1_interrupt_acknowledge &= !(en & self.sa1_interrupt_enable);
                self.sa1_interrupt_trigger |= en;
            }
            (0x2201, SNES) => {
                // SIE - Enable interrupt
                let irq_enable = val & 0x80 > 0;
                let char_dma_irq_enable = val & 0x20 > 0;

                if !replace(&mut self.irq_enable, irq_enable) && irq_enable {
                    todo!("SA-1 irq handling")
                }
                if !replace(&mut self.char_dma_irq_enable, char_dma_irq_enable)
                    && char_dma_irq_enable
                {
                    todo!("SA-1 char dma irq handling")
                }
            }
            (0x2202, SNES) => {
                // SIC - Clear interrupt
                // TODO
            }
            (0x2203 | 0x2204, SNES) => {
                // CRV - Reset vector
                let mut vec = self.reset_vector_override.to_le_bytes();
                vec[usize::from(!id & 1)] = val;
                self.reset_vector_override = u16::from_le_bytes(vec);
            }
            (0x2209, SA1) => {
                // SCNT - Control SNES from SA-1
                self.snes_control_flags = val;
            }
            (0x220a, SA1) => {
                // CIE - SNES Enable Interrupt
                self.sa1_interrupt_acknowledge &=
                    !(!self.sa1_interrupt_enable & val & self.sa1_interrupt_trigger);
                self.sa1_interrupt_enable = val & 0xf0;
            }
            (0x220b, SA1) => {
                // CIC - SA-1 Interrupt Acknowledge
                self.sa1_interrupt_acknowledge = val & 0xf0;
                self.sa1_interrupt_trigger &= !self.sa1_interrupt_acknowledge;
            }
            (0x2220..=0x2223, SNES) => {
                // CXB/DXB/EXB/FXB - Set Bank ROM mapping
                self.blocks[usize::from(id & 4)] = Block::new((id & 4) as u8, val);
            }
            (0x2224, SNES) | (0x2225, SA1) => {
                // BMAPS / BMAP - Set BW-Ram mapping
                self.bwram_map[INTERNAL as usize] = val & 0x1f;
            }
            (0x2226..=0x222a, _) => {
                // Write Protection Registers
                // TODO: no emulator known to me is implementing this. Check why
            }
            _ => todo!("write SA-1 io port {id:04x}"),
        }
    }

    pub fn read<const INTERNAL: bool>(&mut self, addr: Addr24) -> Result<Option<u8>, u32> {
        if addr.bank & 0x40 == 0 {
            match addr.addr {
                0x0000..=0x07ff if INTERNAL => {
                    Ok(Some(self.iram[usize::from(addr.addr) & (IRAM_SIZE - 1)]))
                }
                0x2200..=0x23ff => Ok(Some(self.read_io::<INTERNAL>(addr.addr))),
                0x3000..=0x37ff => Ok(Some(self.iram[usize::from(addr.addr) & (IRAM_SIZE - 1)])),
                0x6000..=0x7fff => Ok(Some(
                    self.bwram[(u32::from(addr.addr & 0x1fff)
                        | (u32::from(self.bwram_map[INTERNAL as usize]) << 13))
                        as usize],
                )),
                0x8000..=0xffff => Err(self.lorom_addr(addr)),
                _ => Ok(None),
            }
        } else if addr.bank & 0x80 == 0 {
            Ok(if addr.bank & 0x30 == 0 {
                Some(self.bwram[(usize::from(addr.bank & 3) << 16) | usize::from(addr.addr)])
            } else {
                None
            })
        } else {
            Err(self.hirom_addr(addr))
        }
    }

    pub fn write<const INTERNAL: bool>(&mut self, addr: Addr24, val: u8) {
        if addr.bank & 0x40 == 0 {
            match addr.addr {
                0x0000..=0x07ff if INTERNAL => {
                    self.iram[usize::from(addr.addr) & (IRAM_SIZE - 1)] = val
                }
                0x2200..=0x23ff => self.write_io::<INTERNAL>(addr.addr, val),
                0x3000..=0x37ff => self.iram[usize::from(addr.addr) & (IRAM_SIZE - 1)] = val,
                0x6000..=0x7fff => {
                    self.bwram[(u32::from(addr.addr & 0x1fff)
                        | (u32::from(self.bwram_map[INTERNAL as usize]) << 13))
                        as usize] = val
                }
                _ => (),
            }
        } else if addr.bank & 0x80 == 0 {
            if addr.bank & 0x30 == 0 {
                self.bwram[(usize::from(addr.bank & 3) << 16) | usize::from(addr.addr)] = val
            }
        }
    }
}

pub struct AccessTypeSa1;

impl<B: crate::backend::AudioBackend, FB: crate::backend::FrameBuffer> AccessType<B, FB>
    for AccessTypeSa1
{
    fn read<D: Data>(device: &mut Device<B, FB>, mut addr: Addr24) -> D {
        let mut arr: D::Arr = Default::default();
        let mut open_bus = device.open_bus;
        for v in arr.as_mut() {
            let sa1 = device.cartridge.as_mut().unwrap().sa1_mut();
            let res = Sa1::read::<true>(sa1, addr);
            *v = res
                .map(|v| v.unwrap_or(open_bus))
                .unwrap_or_else(|addr| device.cartridge.as_mut().unwrap().read_rom(addr));
            open_bus = *v;
            addr.addr = addr.addr.wrapping_add(1);
        }
        D::from_bytes(&arr)
    }

    fn write<D: Data>(device: &mut Device<B, FB>, mut addr: Addr24, val: D) {
        let sa1 = device.cartridge.as_mut().unwrap().sa1_mut();
        for &v in val.to_bytes().as_ref().iter() {
            sa1.write::<true>(addr, v);
            addr.addr = addr.addr.wrapping_add(1);
        }
    }

    fn cpu(device: &Device<B, FB>) -> &Cpu {
        &device.cartridge.as_ref().unwrap().sa1_ref().cpu
    }

    fn cpu_mut(device: &mut Device<B, FB>) -> &mut Cpu {
        &mut device.cartridge.as_mut().unwrap().sa1_mut().cpu
    }
}

impl<'a, B: crate::backend::AudioBackend, FB: crate::backend::FrameBuffer>
    DeviceAccess<'a, AccessTypeSa1, B, FB>
{
    pub fn sa1(&self) -> &Sa1 {
        self.0.cartridge.as_ref().unwrap().sa1_ref()
    }

    pub fn sa1_mut(&mut self) -> &mut Sa1 {
        self.0.cartridge.as_mut().unwrap().sa1_mut()
    }

    pub fn run_cpu<const N: u16>(&mut self) {
        let sa1 = self.sa1_mut();
        let needs_refresh = sa1.ahead_cycles <= 0;
        sa1.ahead_cycles -= i32::from(N);
        if needs_refresh {
            // > WAI/HALT stops the CPU until an exception (usually an IRQ or NMI) request occurs
            // > in case of IRQs this works even if IRQs are disabled (via I=1).
            // source: FullSNES
            if sa1.cpu.wait_mode || sa1.control_flags & 0x60 > 0 {
                sa1.cpu.wait_mode &= !sa1.shall_nmi && !sa1.shall_irq;
                sa1.ahead_cycles += 1;
                return;
            }
            let cycles = if sa1.shall_nmi {
                sa1.shall_nmi = false;
                self.nmi()
            } else if sa1.shall_irq && !sa1.cpu.regs.status.has(crate::cpu::Status::IRQ_DISABLE) {
                sa1.shall_irq = false;
                self.irq()
            } else {
                self.dispatch_instruction() * 6
            };
            self.sa1_mut().ahead_cycles += cycles as i32;
        }
    }
}
