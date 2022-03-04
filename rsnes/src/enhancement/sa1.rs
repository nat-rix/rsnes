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
    instr::AccessType,
};
use save_state_macro::*;

const IRAM_SIZE: usize = 0x800;
const BWRAM_SIZE: usize = 0x40000;

#[derive(Debug, Default, Clone, Copy, InSaveState)]
struct Block {
    hirom_bank: u8,
    lorom_bank: u8,
    lorom_mask: u8,
}

impl Block {
    pub const fn new(id: u8, val: u8) -> Self {
        let bank = (val & 0b111) << 4;
        let b = val & 128 > 0;
        Self {
            hirom_bank: bank,
            lorom_bank: if b { bank } else { 0 },
            lorom_mask: if b { 15 } else { (id << 5) | 15 },
        }
    }

    pub fn lorom(&self, addr: Addr24) -> u32 {
        (addr.addr & 0x7fff) as u32
            | ((((addr.bank & self.lorom_mask) | self.lorom_bank) as u32) << 16)
    }

    pub fn hirom(&self, addr: Addr24) -> u32 {
        addr.addr as u32 | ((((addr.bank & 15) | self.hirom_bank) as u32) << 16)
    }
}

#[derive(Debug, DefaultByNew, Clone, InSaveState)]
pub struct Sa1 {
    iram: [u8; IRAM_SIZE],
    bwram: [u8; BWRAM_SIZE],
    blocks: [Block; 4],
    cpu: Cpu,
}

impl Sa1 {
    pub const fn new() -> Self {
        Self {
            iram: [0; IRAM_SIZE],
            bwram: [0; BWRAM_SIZE],
            blocks: [
                Block::new(0, 0), // Set Super MMC Bank C
                Block::new(0, 1), // Set Super MMC Bank D
                Block::new(0, 2), // Set Super MMC Bank E
                Block::new(0, 3), // Set Super MMC Bank F
            ],
            cpu: Cpu::new(),
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

    pub fn read_io(&mut self, id: u16) -> u8 {
        todo!("io port {id:04x}")
    }

    pub fn write_io(&mut self, id: u16, val: u8) {
        todo!("io port {id:04x}")
    }

    pub fn read<const INTERNAL: bool>(&mut self, addr: Addr24) -> Result<Option<u8>, u32> {
        if addr.bank & 0x40 == 0 {
            match addr.addr {
                0x0000..=0x07ff if INTERNAL => {
                    Ok(Some(self.iram[usize::from(addr.addr) & (IRAM_SIZE - 1)]))
                }
                0x2200..=0x23ff => Ok(Some(self.read_io(addr.addr))),
                0x3000..=0x37ff => Ok(Some(self.iram[usize::from(addr.addr) & (IRAM_SIZE - 1)])),
                0x6000..=0x7fff => Ok(Some(self.bwram[usize::from(addr.addr & 0x1fff)])),
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
                0x2200..=0x23ff => self.write_io(addr.addr, val),
                0x3000..=0x37ff => self.iram[usize::from(addr.addr) & (IRAM_SIZE - 1)] = val,
                0x6000..=0x7fff => self.bwram[usize::from(addr.addr & 0x1fff)] = val,
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
            let res = sa1.read::<true>(addr);
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
}
