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

#[derive(Debug, Clone, InSaveState)]
struct Vectors {
    vectors: [u16; 3],
    overrides: [u16; 2],
}

impl Vectors {
    pub const fn new() -> Self {
        Self {
            vectors: [0; 3],
            overrides: [0; 2],
        }
    }

    pub fn set_vector(&mut self, id: u16, val: u8) {
        let vector = &mut self.vectors[usize::from((id.wrapping_sub(3) >> 1) & 3)];
        let mut arr = vector.to_le_bytes();
        arr[usize::from(!id & 1)] = val;
        *vector = u16::from_le_bytes(arr);
    }

    pub fn set_override(&mut self, id: u16, val: u8) {
        let vector = &mut self.overrides[usize::from(id >> 1) & 1];
        let mut arr = vector.to_le_bytes();
        arr[usize::from(id & 1)] = val;
        *vector = u16::from_le_bytes(arr);
    }

    pub const fn get_reset(&self) -> u16 {
        self.vectors[0]
    }

    pub const fn get_nmi(&self) -> u16 {
        self.vectors[1]
    }

    pub const fn get_irq(&self) -> u16 {
        self.vectors[2]
    }

    pub const fn get_override_nmi(&self) -> u16 {
        self.overrides[0]
    }

    pub const fn get_override_irq(&self) -> u16 {
        self.overrides[1]
    }
}

#[derive(Debug, Clone, Copy, InSaveState)]
pub struct DmaDirection(u8);

impl DmaDirection {
    pub const fn new(val: u8) -> Self {
        Self(val)
    }

    pub const fn is_src_rom(&self) -> bool {
        self.0 & 3 == 0
    }

    pub const fn is_src_bwram(&self) -> bool {
        self.0 & 3 == 1
    }

    pub const fn is_src_iram(&self) -> bool {
        self.0 & 3 == 2
    }

    pub const fn is_dst_bwram(&self) -> bool {
        self.0 & 4 > 0
    }
}

#[derive(Debug, Clone, InSaveState)]
pub struct DmaInfo {
    enable: bool,
    direction: DmaDirection,
    is_automatic: bool,
    char_conversion: bool,
    priority: bool,
    color_bits: u8,
    vram_width: u8,
    terminate: bool,
}

impl DmaInfo {
    pub const fn new() -> Self {
        Self {
            enable: false,
            direction: DmaDirection::new(0),
            is_automatic: false,
            char_conversion: false,
            priority: false,
            color_bits: 8,
            vram_width: 1,
            terminate: false,
        }
    }
}

#[derive(Debug, Clone, InSaveState)]
pub struct Timer {
    // 1: horizontal
    // 2: vertical
    interrupt: u8,
    is_linear: bool,
    h: u16,
    v: u16,
    hmax: u16,
    vmax: u16,
}

impl Timer {
    pub const fn new() -> Self {
        Self {
            interrupt: 0,
            is_linear: false,
            h: 0,
            v: 0,
            hmax: 0,
            vmax: 0,
        }
    }

    pub fn set_max(&mut self, val: u8, is_high: bool, is_h: bool) {
        let hv = if is_h { &mut self.h } else { &mut self.v };
        let mut bytes = hv.to_le_bytes();
        bytes[usize::from(is_high)] = val;
        *hv = u16::from_le_bytes(bytes) & 0x1f;
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
    vectors: Vectors,
    snes_control_flags: u8,
    control_flags: u8,
    bwram_map: [u8; 2],
    bwram_map_bits: bool,
    bwram_2bits: bool,
    dma: DmaInfo,
    timer: Timer,

    // SA-1-side interrupt flags
    // 0x10: NMI from SNES
    // 0x20: DMA IRQ
    // 0x40: Timer IRQ
    // 0x80: IRQ from SNES
    sa1_interrupt_enable: u8,
    sa1_interrupt_acknowledge: u8,
    sa1_interrupt_trigger: u8,

    // SNES-side interrupt flags
    // 0x20: IRQ from Character conversion DMA
    // 0x80: IRQ from SA-1
    snes_interrupt_enable: u8,
    snes_interrupt_acknowledge: u8,
    snes_interrupt_trigger: u8,
    snes_irq_pin: bool,
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
            vectors: Vectors::new(),
            snes_control_flags: 0,
            control_flags: 0x20,
            bwram_map: [0; 2],
            bwram_map_bits: false,
            bwram_2bits: false,
            dma: DmaInfo::new(),
            timer: Timer::new(),

            sa1_interrupt_enable: 0,
            sa1_interrupt_acknowledge: 0,
            sa1_interrupt_trigger: 0,

            snes_interrupt_enable: 0,
            snes_interrupt_acknowledge: 0,
            snes_interrupt_trigger: 0,
            snes_irq_pin: false,
        }
    }

    pub fn reset(&mut self) {
        // TODO: correctly implement resetting
        *self = Self::new()
    }

    pub fn cpu_mut(&mut self) -> &mut Cpu {
        &mut self.cpu
    }

    pub const fn irq_pin(&self) -> bool {
        self.snes_irq_pin
    }

    pub const fn get_override_nmi(&self) -> Option<u16> {
        if self.snes_control_flags & 0x10 > 0 {
            Some(self.vectors.get_override_nmi())
        } else {
            None
        }
    }

    pub const fn get_override_irq(&self) -> Option<u16> {
        if self.snes_control_flags & 0x40 > 0 {
            Some(self.vectors.get_override_irq())
        } else {
            None
        }
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

    fn read_bwram_bits_with<const A1: u8, const A2: u8, const M1: u32, const M2: u8>(
        &self,
        addr: u32,
    ) -> u8 {
        let val = self.bwram[(addr >> A2) as usize];
        (val >> ((addr & M1) << A1)) & M2
    }

    fn write_bwram_bits_with<const A1: u8, const A2: u8, const M1: u32, const M2: u8>(
        &mut self,
        addr: u32,
        val: u8,
    ) {
        let r = &mut self.bwram[(addr >> A2) as usize];
        let s = (addr & M1) << A1;
        *r = (*r & !(M2 << s)) | ((val & M2) << s)
    }

    fn read_bwram_bits(&self, addr: u32) -> u8 {
        if self.bwram_2bits {
            self.read_bwram_bits_with::<1, 2, 3, 3>(addr)
        } else {
            self.read_bwram_bits_with::<2, 1, 1, 15>(addr)
        }
    }

    fn write_bwram_bits(&mut self, addr: u32, val: u8) {
        if self.bwram_2bits {
            self.write_bwram_bits_with::<1, 2, 3, 3>(addr, val)
        } else {
            self.write_bwram_bits_with::<2, 1, 1, 15>(addr, val)
        }
    }

    fn get_bwram_small<const INTERNAL: bool>(&self, addr: Addr24) -> u32 {
        (u32::from(self.bwram_map[INTERNAL as usize]) << 13) | u32::from(addr.addr & 0x1fff)
    }

    fn read_bwram_small<const INTERNAL: bool>(&self, addr: Addr24) -> u8 {
        let addr = self.get_bwram_small::<INTERNAL>(addr);
        if INTERNAL && self.bwram_map_bits {
            return self.read_bwram_bits(addr);
        }
        self.bwram[(addr & 0x3_ffff) as usize]
    }

    fn write_bwram_small<const INTERNAL: bool>(&mut self, addr: Addr24, val: u8) {
        let addr = self.get_bwram_small::<INTERNAL>(addr);
        if INTERNAL && self.bwram_map_bits {
            return self.write_bwram_bits(addr, val);
        }
        self.bwram[(addr & 0x3_ffff) as usize] = val
    }

    pub fn read_io<const INTERNAL: bool>(&mut self, id: u16) -> u8 {
        const SA1: bool = true;
        const SNES: bool = false;
        match (id, INTERNAL) {
            (0x2300, SNES) => {
                // SCNT - SNES Control flags
                // TODO: IRQ from Character Conversion DMA
                // TODO: IRQ from SA-1 to SNES
                (self.snes_control_flags & 0x5f) | (self.snes_interrupt_trigger & 0xa0)
            }
            (0x2301, SA1) => {
                // CFR - SA-1 Control flags
                (self.control_flags & 0xf) | self.sa1_interrupt_trigger
            }
            _ => todo!(
                "read SA-1 io port {id:04x} from {} SA-1",
                ["outside", "inside"][INTERNAL as usize]
            ),
        }
    }

    pub fn write_io<const INTERNAL: bool>(&mut self, id: u16, val: u8) {
        const SA1: bool = true;
        const SNES: bool = false;
        match (id, INTERNAL) {
            (0x2200, SNES) => {
                // CCNT - Control SA-1 from SNES
                if replace(&mut self.control_flags, val) & !val & 0x20 > 0 {
                    self.cpu.regs.pc = Addr24::new(0, self.vectors.get_reset())
                }
                let en = val & 0x90;
                self.sa1_interrupt_acknowledge &= !(en & self.sa1_interrupt_enable);
                self.sa1_interrupt_trigger |= en;
            }
            (0x2201, SNES) => {
                // SIE - Enable interrupt
                let irq = !replace(&mut self.snes_interrupt_enable, val)
                    & val
                    & self.snes_interrupt_trigger;
                if irq & 0x80 > 0 {
                    self.snes_interrupt_acknowledge &= 0x7f;
                    self.snes_irq_pin = true;
                }
                if irq & 0x20 > 0 {
                    self.snes_interrupt_acknowledge &= !0x20;
                    self.snes_irq_pin = true;
                }
            }
            (0x2202, SNES) => {
                // SIC - Clear interrupt
                self.snes_interrupt_acknowledge = val;
                self.snes_interrupt_trigger &= !val;
                self.snes_irq_pin &= self.snes_interrupt_trigger & 0xa0 > 0;
            }
            (0x2203..=0x2208, SNES) => {
                // CRV/CNV/CIV - Interrupt vectors
                self.vectors.set_vector(id, val)
            }
            (0x2209, SA1) => {
                // SCNT - Control SNES from SA-1
                self.snes_control_flags = val;
                if val & 0x80 > 0 {
                    self.snes_interrupt_trigger |= 0x80;
                    if self.snes_interrupt_enable & 0x80 > 0 {
                        self.snes_interrupt_acknowledge &= 0x7f;
                        self.snes_irq_pin = true;
                    }
                }
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
            (0x220c..=0x220f, SA1) => {
                // SNV/SIV - SNES override interrupt vectors
                self.vectors.set_override(id, val)
            }
            (0x2210, SA1) => {
                // TMC - Timer Control
                self.timer.interrupt = val & 3;
                self.timer.is_linear = val & 0x80 > 0;
            }
            (0x2211, SA1) => {
                // CTR - Reset Timer
                self.timer.h = 0;
                self.timer.v = 0;
            }
            (0x2212..=0x2215, SA1) => {
                // HVNC/VCNT - Set Timer maximum
                self.timer.set_max(val, id & 1 > 0, id & 2 > 0)
            }
            (0x2220..=0x2223, SNES) => {
                // CXB/DXB/EXB/FXB - Set Bank ROM mapping
                self.blocks[usize::from(id & 3)] = Block::new((id & 3) as u8, val);
            }
            (0x2224, SNES) => {
                // BMAPS - Set SNES-side BW-Ram mapping
                self.bwram_map[0] = val & 0x1f;
            }
            (0x2225, SA1) => {
                // BMAP - Set SA1-side BW-Ram mapping
                self.bwram_map[1] = val & 0x7f;
                self.bwram_map_bits = val & 0x80 > 0;
            }
            (0x2226..=0x222a, _) => {
                // Write Protection Registers
                // TODO: no emulator known to me is implementing this. Check why
            }
            (0x2230, SA1) => {
                // DCNT - DMA Control
                self.dma.direction = DmaDirection::new(val);
                self.dma.is_automatic = val & 0x10 > 0;
                self.dma.char_conversion = val & 0x20 > 0;
                self.dma.priority = val & 0x40 > 0;
                self.dma.enable = val & 0x80 > 0;
            }
            (0x2231, _) => {
                // CDMA - Character Conversion DMA Parameters
                // TODO: what happens, when `color_bits = 1`?
                // TODO: what happens, when `vram_width = 64 or 128`?
                self.dma.color_bits = 1 << (!val & 3);
                self.dma.vram_width = 1 << ((val >> 2) & 7);
                self.dma.terminate = val & 0x80 > 0;
            }
            (0x223f, SA1) => {
                // BBF - BW-Ram bitmap mode
                self.bwram_2bits = val & 0x80 > 0;
            }
            (0x2261 | 0x2262, _) => (), // Undocumented
            _ => todo!(
                "write SA-1 io port {id:04x} from {} SA-1",
                ["outside", "inside"][INTERNAL as usize]
            ),
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
                0x6000..=0x7fff => Ok(Some(self.read_bwram_small::<INTERNAL>(addr))),
                0x8000..=0xffff => Err(self.lorom_addr(addr)),
                _ => Ok(None),
            }
        } else if addr.bank & 0x80 == 0 {
            Ok(match addr.bank & 0x30 {
                0x00 => {
                    Some(self.bwram[(usize::from(addr.bank & 3) << 16) | usize::from(addr.addr)])
                }
                0x20 => Some(
                    self.read_bwram_bits((u32::from(addr.bank & 15) << 16) | u32::from(addr.bank)),
                ),
                _ => None,
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
                0x6000..=0x7fff => self.write_bwram_small::<INTERNAL>(addr, val),
                _ => (),
            }
        } else if addr.bank & 0x80 == 0 {
            match addr.bank & 0x30 {
                0x00 => {
                    self.bwram[(usize::from(addr.bank & 3) << 16) | usize::from(addr.addr)] = val
                }
                0x20 => self.write_bwram_bits(
                    (u32::from(addr.bank & 15) << 16) | u32::from(addr.bank),
                    val,
                ),
                _ => (),
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
            if sa1.cpu.wait_mode || sa1.control_flags & 0x60 != 0 {
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
