//! Utilities to read a cartridge into memory
//!
//! # Literature
//!
//! - the [super famicom wiki page](https://wiki.superfamicom.org/memory-mapping)
//! - http://patrickjohnston.org/ASM/ROM data/snestek.htm

use std::convert::TryInto;

use crate::cpu::Addr24;

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

#[derive(Debug, Clone)]
pub struct Header {
    name: String,
    speed: u8,
    rom_type: RomType,
    extended: OptExtendedHeader,
    is_fast: bool,
    coprocessor: u8,
    chips: u8,
    rom_size: u32,
    ram_size: u32,
    country: u8,
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
                version,
            },
            score,
        ))
    }
}

pub struct Cartridge {
    header: Header,
    is_lorom: bool,
    rom: Vec<u8>,
    ram: Vec<u8>,
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
        for (addr, is_lorom) in [(0x7fb0, true), (0xffb0, false), (0x40ffb0, false)] {
            if bytes.len() >= addr + 80 {
                if let Some((new, score)) = Header::from_bytes(&bytes[addr..addr + 80]) {
                    if header.as_ref().map(|(_, s, _)| score > *s).unwrap_or(true) {
                        header = Some((new, score, is_lorom));
                    }
                }
            }
        }
        let (header, _score, is_lorom) = header.ok_or(ReadRomError::NoSuitableHeader)?;

        let mut rom = vec![0u8; usize::max(header.rom_size as usize, bytes.len())];
        for chunk in rom.chunks_mut(bytes.len()) {
            chunk.copy_from_slice(&bytes[..chunk.len()])
        }
        let ram_size = header.ram_size;

        Ok(Self {
            rom,
            ram: vec![0; ram_size as usize],
            header,
            is_lorom,
        })
    }

    pub const fn header(&self) -> &Header {
        &self.header
    }

    fn read_ram(&self, index: usize) -> u8 {
        // this is safe, because `n & (len - 1) < len` for all n
        *unsafe { self.ram.get_unchecked(index & (self.ram.len() - 1)) }
    }

    fn read_rom(&self, index: usize) -> u8 {
        // this is safe, because `n & (len - 1) < len` for all n
        *unsafe { self.rom.get_unchecked(index & (self.rom.len() - 1)) }
    }

    /// Read byte from cartridge
    pub fn read_byte_unchecked(&self, addr: Addr24) -> Option<u8> {
        if self.is_lorom {
            if addr.is_lower_half() {
                if (0x70..0x7e).contains(&addr.bank) || addr.bank >= 0xf0 {
                    Some(self.read_ram(((addr.bank as usize & 0xf) << 15) | addr.addr as usize))
                } else {
                    None
                }
            } else if addr.bank >= 0x40 {
                Some(self.read_rom(
                    (((addr.bank as usize) << 15) | addr.addr as usize) & (self.rom.len() - 1),
                ))
            } else {
                None
            }
        } else {
            if !addr.is_lower_half() {
                if addr.bank >= 0x40 {
                    Some(self.read_rom(((addr.bank as usize & 0x3f) << 16) | addr.addr as usize))
                } else {
                    None
                }
            } else if addr.bank & 0x7f < 0x40 && addr.is_lower_half() && addr.addr >= 0x6000 {
                Some(
                    self.read_ram(
                        ((addr.bank as usize & 0x3f) << 13) | (addr.addr as usize & 0x1fff),
                    ),
                )
            } else {
                None
            }
        }
    }
}
