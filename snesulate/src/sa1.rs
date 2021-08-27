//! SA-1 Coprocessor handling types
//!
//! # Literature
//!
//! - <https://wiki.superfamicom.org/sa-1>
//! - <https://wiki.superfamicom.org/sa-1-registers>
//! - <https://wiki.superfamicom.org/uploads/assembly-programming-manual-for-w65c816.pdf>
//! - <https://problemkaputt.de/fullsnes.htm>

use crate::device::{Addr24, Data};

const IRAM_SIZE: usize = 0x800;

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone)]
pub struct Sa1 {
    pub(crate) iram: [u8; IRAM_SIZE],
    blocks: [Block; 4],
}

impl Sa1 {
    pub const fn new() -> Self {
        Self {
            iram: [0; IRAM_SIZE],
            blocks: [
                Block::new(0, 0), // Set Super MMC Bank C
                Block::new(0, 1), // Set Super MMC Bank D
                Block::new(0, 2), // Set Super MMC Bank E
                Block::new(0, 3), // Set Super MMC Bank F
            ],
        }
    }

    pub const fn iram_ref(&self) -> &[u8; IRAM_SIZE] {
        &self.iram
    }

    pub fn iram_mut(&mut self) -> &mut [u8; IRAM_SIZE] {
        &mut self.iram
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
}
