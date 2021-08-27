//! Utilities to read a cartridge into memory
//!
//! # Literature
//!
//! - the [super famicom wiki page](https://wiki.superfamicom.org/memory-mapping)
//! - <http://patrickjohnston.org/ASM/ROM data/snestek.htm>

use std::convert::TryInto;

use crate::device::{Addr24, Data};
use crate::sa1::Sa1;

const MINIMUM_SIZE: usize = 0x8000;

fn split_byte(byte: u8) -> (u8, u8) {
    (byte >> 4, byte & 15)
}

#[derive(Debug)]
pub enum ReadRomError {
    TooSmall(usize),
    AlignError(usize),
    NoSuitableHeader,
}

impl std::fmt::Display for ReadRomError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::TooSmall(size) => write!(f, "file too small ({} < {})", size, MINIMUM_SIZE),
            Self::AlignError(size) => {
                write!(f, "file must be a multiple of 512 in length (got {})", size)
            }
            Self::NoSuitableHeader => write!(f, "no suitable header found"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum RomType {
    LoRom,
    HiRom,
    LoRomSDD1,
    LoRomSA1,
    // > ExHiRom only used by "Dai Kaiju Monogatari 2 (JP)" and "Tales of Phantasia (JP)"
    // source: nocache SNES hardware specification
    //         <https://problemkaputt.de/fullsnes.htm>
    ExHiRom,
    HiRomSPC7110,
}

impl RomType {
    fn from_byte(byte: u8) -> Option<RomType> {
        Some(match byte {
            0 => Self::LoRom,
            1 => Self::HiRom,
            2 => Self::LoRomSDD1,
            3 => Self::LoRomSA1,
            5 => Self::ExHiRom,
            10 => Self::HiRomSPC7110,
            _ => return None,
        })
    }

    pub fn to_mapping(&self) -> MemoryMapping {
        match self {
            Self::LoRom => MemoryMapping::LoRom,
            Self::HiRom => MemoryMapping::HiRom,
            Self::LoRomSA1 => MemoryMapping::LoRomSa1 { sa1: Sa1::new() },
            rom_type => todo!("ROM type {:?} not supported yet", rom_type),
        }
    }
}

#[derive(Debug, Clone)]
pub enum MemoryMapping {
    LoRom,
    HiRom,
    LoRomSa1 { sa1: Sa1 },
}

#[derive(Debug, Clone)]
pub struct ExtendedHeader {
    maker: [u8; 2],
    game: [u8; 4],
    flash_size: u32,
    ram_size: u32,
    special_version: u8,
}

#[derive(Debug, Clone)]
pub enum OptExtendedHeader {
    Old { subtype: u8 },
    Later { subtype: u8, header: ExtendedHeader },
    None,
}

#[derive(Debug, Clone, Copy)]
pub enum Coprocessor {
    Dsp,
    Gsu,
    Obc1,
    Sa1,
    Sdd1,
    Srtc,
    Spc7110,
    St01x,
    St018,
    Cx4,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct Header {
    name: String,
    speed: u8,
    rom_type: RomType,
    extended: OptExtendedHeader,
    is_fast: bool,
    coprocessor: Option<Coprocessor>,
    chips: u8,
    rom_size: u32,
    ram_size: u32,
    country: u8,
    checksum: u16,
    version: u8,
}

impl Header {
    pub fn from_bytes(full_bytes: &[u8]) -> Option<(Self, u16)> {
        const VALID_CHAR: u16 = 2;
        const VALID_CHECKSUM_COMPLEMENT: u16 = 32;
        const VALID_SPEED_INDICATION: u16 = 24;
        const KNOWN_COUNTRY: u16 = 10;
        assert_eq!(full_bytes.len(), 80);

        let bytes = &full_bytes[16..];
        let mut name = String::with_capacity(21);
        let mut score = 0;
        let mut len = 21;
        for c in &bytes[..21] {
            if matches!(c, 0x20..=0x7e) {
                name.push(*c as char);
                score += VALID_CHAR
            }
            if c == &b' ' {
                len -= 1
            } else {
                len = 21
            }
        }
        // trim away trailing whitespace
        name.truncate(len);
        let (speed, rom_type) = split_byte(bytes[21]);
        if speed & !1 == 1 {
            score += VALID_SPEED_INDICATION
        }
        let is_fast = speed & 1 == 1;
        let rom_type = RomType::from_byte(rom_type)?;
        let (coprocessor, chips) = split_byte(bytes[22]);
        let rom_size = 0x400u32.wrapping_shl(bytes[23].into());
        let ram_size = 0x400u32.wrapping_shl(bytes[24].into());
        let country = bytes[25];
        if country <= 20 {
            score += KNOWN_COUNTRY
        }
        let developer_id = bytes[26];
        let version = bytes[27];
        let checksum_complement = u16::from_le_bytes(bytes[28..30].try_into().unwrap());
        let checksum = u16::from_le_bytes(bytes[30..32].try_into().unwrap());
        if checksum_complement == !checksum {
            score += VALID_CHECKSUM_COMPLEMENT
        }
        let extended = if developer_id == 51 {
            // later Extended Header
            OptExtendedHeader::Later {
                header: ExtendedHeader {
                    maker: full_bytes[0..2].try_into().unwrap(),
                    game: full_bytes[2..6].try_into().unwrap(),
                    flash_size: 0x400u32.wrapping_shl(full_bytes[12].into()),
                    ram_size: 0x400u32.wrapping_shl(full_bytes[13].into()),
                    special_version: full_bytes[14],
                },
                subtype: full_bytes[15],
            }
        } else if bytes[20] == 0 {
            // Early Extended Header
            OptExtendedHeader::Old {
                subtype: full_bytes[15],
            }
        } else {
            OptExtendedHeader::None
        };
        let coprocessor = match (
            chips,
            coprocessor,
            match extended {
                OptExtendedHeader::Old { subtype } | OptExtendedHeader::Later { subtype, .. } => {
                    subtype
                }
                _ => 0,
            },
        ) {
            (0..=2, 0, _) => None,
            (3.., 0, _) => Some(Coprocessor::Dsp),
            (_, 1, _) => Some(Coprocessor::Gsu),
            (_, 2, _) => Some(Coprocessor::Obc1),
            (_, 3, _) => Some(Coprocessor::Sa1),
            (_, 4, _) => Some(Coprocessor::Sdd1),
            (_, 5, _) => Some(Coprocessor::Srtc),
            (_, 15, 0) => Some(Coprocessor::Spc7110),
            (_, 15, 1) => Some(Coprocessor::St01x),
            (_, 15, 2) => Some(Coprocessor::St018),
            (_, 15, 16) => Some(Coprocessor::Cx4),
            _ => Some(Coprocessor::Unknown),
        };
        Some((
            Self {
                name,
                speed,
                rom_type,
                extended,
                is_fast,
                coprocessor,
                chips,
                rom_size,
                ram_size,
                country,
                checksum,
                version,
            },
            score,
        ))
    }

    pub const fn has_ram(&self) -> bool {
        self.chips != 3 && self.chips != 6 && !(self.chips == 0 && self.coprocessor.is_none())
    }
}

#[derive(Debug, Clone)]
pub struct Cartridge {
    header: Header,
    rom: Vec<u8>,
    ram: Vec<u8>,
    mapping: MemoryMapping,
}

impl Cartridge {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ReadRomError> {
        if bytes.len() < MINIMUM_SIZE {
            return Err(ReadRomError::TooSmall(bytes.len()));
        }
        if bytes.len() & 0x1ff != 0 {
            return Err(ReadRomError::AlignError(bytes.len()));
        }
        let bytes = if bytes.len() & 0x3ff == 0 {
            bytes
        } else {
            &bytes[512..]
        };

        let mut header = None;
        for addr in [0x7fb0, 0xffb0, 0x40ffb0] {
            if bytes.len() >= addr + 80 {
                if let Some((new, score)) = Header::from_bytes(&bytes[addr..addr + 80]) {
                    if header.as_ref().map(|(_, s)| score > *s).unwrap_or(true) {
                        header = Some((new, score));
                    }
                }
            }
        }
        let (header, _score) = header.ok_or(ReadRomError::NoSuitableHeader)?;

        let mut rom = vec![0u8; usize::max(header.rom_size as usize, bytes.len())];
        for chunk in rom.chunks_mut(bytes.len()) {
            chunk.copy_from_slice(&bytes[..chunk.len()])
        }

        let checksum = rom.iter().fold(0u16, |b, i| b.wrapping_add((*i).into()));
        if checksum != header.checksum {
            println!("warning: checksum did not match! Checksum in ROM is {:04x}; Calculated checksum is {:04x}", header.checksum, checksum);
        }

        let ram_size = header.ram_size;

        Ok(Self {
            rom,
            ram: vec![0; ram_size as usize],
            mapping: header.rom_type.to_mapping(),
            header,
        })
    }

    pub const fn header(&self) -> &Header {
        &self.header
    }

    /// Read from the cartridge
    pub fn read<D: Data>(&self, addr: Addr24) -> Option<D> {
        let mask = self.ram.len() - 1;
        match &self.mapping {
            MemoryMapping::LoRom => match (addr.bank, addr.addr) {
                ((0x70..=0x7d) | (0xf0..), 0..=0x7fff) => Some(D::parse(
                    &self.ram,
                    (((addr.bank as usize & 0xf) << 15) | addr.addr as usize) & mask,
                )),
                (0x40.., _) | (_, 0x8000..) => Some(D::parse(
                    &self.rom,
                    ((addr.bank as usize & 0x7f) << 15) | (addr.addr & 0x7fff) as usize,
                )),
                _ => None,
            },
            MemoryMapping::HiRom => match (addr.bank & 0x7f, addr.addr) {
                (0..=0x3f, 0x6000..=0x7fff) => Some(D::parse(
                    &self.ram,
                    (((addr.bank as usize & 0x3f) << 13) | (addr.addr & 0x1fff) as usize) & mask,
                )),
                (0x40.., _) | (_, 0x8000..) => Some(D::parse(
                    &self.rom,
                    ((addr.bank as usize & 0x3f) << 16) | addr.addr as usize,
                )),
                _ => None,
            },
            MemoryMapping::LoRomSa1 { sa1 } => {
                // LoRomSa1
                match (addr.bank, addr.addr) {
                    ((0x00..=0x3f) | (0x80..=0xbf), (0x2200..=0x23ff)) => {
                        todo!("sa1 i/o-ports read access at {}", addr)
                    }
                    ((0x00..=0x3f) | (0x80..=0xbf), (0x3000..=0x37ff)) => {
                        Some(D::parse(sa1.iram_ref(), (addr.addr & 0x7ff) as usize))
                    }
                    ((0x00..=0x3f) | (0x80..=0xbf), (0x6000..=0x7fff)) => {
                        todo!("sa1 8kb bw-ram read access at {}", addr)
                    }
                    ((0x00..=0x3f) | (0x80..=0xbf), (0x8000..=0xffff)) => {
                        Some(D::parse(&self.rom, sa1.lorom_addr(addr) as usize))
                    }
                    (0x40..=0x4f, _) => todo!("sa1 256kb blocks read access at {}", addr),
                    (0xc0..=0xff, _) => Some(D::parse(&self.rom, sa1.hirom_addr(addr) as usize)),
                    _ => None,
                }
            }
        }
    }

    /// Write to the cartridge
    pub fn write<D: Data>(&mut self, addr: Addr24, value: D) {
        let mask = self.ram.len() - 1;
        match &mut self.mapping {
            MemoryMapping::LoRom => match (addr.bank, addr.addr) {
                ((0x70..=0x7d) | (0xf0..), 0..=0x7fff) => value.write_to(
                    &mut self.ram,
                    (((addr.bank as usize & 0xf) << 15) | addr.addr as usize) & mask,
                ),
                (0x40.., _) | (_, 0x8000..) => value.write_to(
                    &mut self.rom,
                    ((addr.bank as usize & 0x7f) << 15) | (addr.addr & 0x7fff) as usize,
                ),
                _ => (),
            },
            MemoryMapping::HiRom => match (addr.bank & 0x7f, addr.addr) {
                (0..=0x3f, 0x6000..=0x7fff) => value.write_to(
                    &mut self.ram,
                    (((addr.bank as usize & 0x3f) << 13) | (addr.addr & 0x1fff) as usize) & mask,
                ),
                (0x40.., _) | (_, 0x8000..) => value.write_to(
                    &mut self.rom,
                    ((addr.bank as usize & 0x3f) << 16) | addr.addr as usize,
                ),
                _ => (),
            },
            MemoryMapping::LoRomSa1 { sa1 } => {
                // LoRomSa1
                match (addr.bank, addr.addr) {
                    ((0x00..=0x3f) | (0x80..=0xbf), (0x2200..=0x23ff)) => {
                        for (i, b) in value.to_bytes().as_ref().iter().cloned().enumerate() {
                            match addr.addr.wrapping_add(i as u16) {
                                0x2200 => {
                                    // TODO: fully implement this:
                                    //   - interrupt flag 0x80 missing
                                    //   - ready flag 0x40 missing
                                    //   - NMI flag 0x10 missing
                                    sa1.set_input(b & 15);
                                    if b & 0x20 > 0 {
                                        sa1.reset();
                                    }
                                }
                                _ => todo!("sa1 i/o-ports write access at {}", addr),
                            }
                        }
                    }
                    ((0x00..=0x3f) | (0x80..=0xbf), (0x3000..=0x37ff)) => {
                        value.write_to(sa1.iram_mut(), (addr.addr & 0x7ff) as usize)
                    }
                    ((0x00..=0x3f) | (0x80..=0xbf), (0x6000..=0x7fff)) => {
                        todo!("sa1 8kb bw-ram write access at {}", addr)
                    }
                    ((0x00..=0x3f) | (0x80..=0xbf), (0x8000..=0xffff)) => {
                        value.write_to(&mut self.rom, sa1.lorom_addr(addr) as usize)
                    }
                    (0x40..=0x4f, _) => todo!("sa1 256kb blocks write access at {}", addr),
                    (0xc0..=0xff, _) => {
                        value.write_to(&mut self.rom, sa1.hirom_addr(addr) as usize)
                    }
                    _ => (),
                }
            }
        }
    }
}
