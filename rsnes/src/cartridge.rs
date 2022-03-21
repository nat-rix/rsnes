//! Utilities to read a cartridge into memory
//!
//! # Literature
//!
//! - the [super famicom wiki page](https://wiki.superfamicom.org/memory-mapping)
//! - <http://patrickjohnston.org/ASM/ROM data/snestek.htm>

use std::convert::TryInto;

use crate::{
    device::{Addr24, Data},
    enhancement::{sa1::Sa1, Dsp, DspVersion},
    timing::Cycles,
};
use save_state::{SaveStateDeserializer, SaveStateSerializer};
use save_state_macro::*;

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

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
enum RomType {
    LoRom = 0,
    HiRom = 1,
    LoRomSDD1 = 2,
    LoRomSA1 = 3,
    // > ExHiRom only used by "Dai Kaiju Monogatari 2 (JP)" and "Tales of Phantasia (JP)"
    // source: nocache SNES hardware specification
    //         <https://problemkaputt.de/fullsnes.htm>
    ExHiRom = 5,
    HiRomSPC7110 = 10,
}

impl RomType {
    const fn from_byte(byte: u8) -> Option<RomType> {
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

impl save_state::InSaveState for RomType {
    fn serialize(&self, state: &mut SaveStateSerializer) {
        (*self as u8).serialize(state)
    }

    fn deserialize(&mut self, state: &mut SaveStateDeserializer) {
        let mut i: u8 = 0;
        i.deserialize(state);
        *self = Self::from_byte(i).unwrap_or_else(|| panic!("unknown enum discriminant {}", i))
    }
}

impl Default for RomType {
    fn default() -> Self {
        Self::from_byte(0).unwrap()
    }
}

#[derive(Debug, Default, Clone, InSaveState)]
pub struct ExtendedHeader {
    maker: [u8; 2],
    game: [u8; 4],
    flash_size: u32,
    ram_size: u32,
    special_version: u8,
}

#[derive(Debug, Clone)]
pub enum OptExtendedHeader {
    None,
    Old { subtype: u8 },
    Later { subtype: u8, header: ExtendedHeader },
}

impl save_state::InSaveState for OptExtendedHeader {
    fn serialize(&self, state: &mut SaveStateSerializer) {
        let i: u8 = match self {
            Self::None => 0,
            Self::Old { .. } => 1,
            Self::Later { .. } => 2,
        };
        i.serialize(state);
        match self {
            Self::None => (),
            Self::Old { subtype } => subtype.serialize(state),
            Self::Later { subtype, header } => {
                subtype.serialize(state);
                header.serialize(state);
            }
        }
    }

    fn deserialize(&mut self, state: &mut SaveStateDeserializer) {
        let mut i: u8 = 0;
        i.deserialize(state);
        *self = match i {
            0 => Self::None,
            1 => {
                let mut subtype: u8 = 0;
                subtype.deserialize(state);
                Self::Old { subtype }
            }
            2 => {
                let mut subtype: u8 = 0;
                subtype.deserialize(state);
                let mut header = ExtendedHeader::default();
                header.deserialize(state);
                Self::Later { subtype, header }
            }
            _ => panic!("unknown enum discriminant {}", i),
        }
    }
}

impl Default for OptExtendedHeader {
    fn default() -> Self {
        Self::None
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum Coprocessor {
    Dsp = 0,
    Gsu = 1,
    Obc1 = 2,
    Sa1 = 3,
    Sdd1 = 4,
    Srtc = 5,
    Spc7110 = 6,
    St01x = 7,
    St018 = 8,
    Cx4 = 9,
    Unknown = 0xff,
}

impl save_state::InSaveState for Coprocessor {
    fn serialize(&self, state: &mut SaveStateSerializer) {
        (*self as u8).serialize(state)
    }

    #[allow(non_upper_case_globals)]
    fn deserialize(&mut self, state: &mut SaveStateDeserializer) {
        let mut i: u8 = 0;
        i.deserialize(state);
        macro_rules! deser {
            ($($val:ident),*) => {{
                $(const $val: u8 = Coprocessor::$val as u8;)*
                match i {
                    $($val => Self::$val,)*
                    _ => Self::Unknown,
                }
            }};
        }
        *self = deser!(Dsp, Gsu, Obc1, Sa1, Sdd1, Srtc, Spc7110, St01x, St018, Cx4)
    }
}

impl Default for Coprocessor {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Default, Clone, InSaveState)]
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
        let mut score = 0;
        let mut name = bytes[..21]
            .iter()
            .filter(|c| (b' '..=b'~').contains(c))
            .skip_while(|&&c| c == b' ')
            .map(|&c| if c == b'\\' { '¥' } else { c.into() })
            .collect::<String>()
            .to_string();
        while name.ends_with(' ') {
            name.truncate(name.len() - 1)
        }
        score += name.len() as u16 * VALID_CHAR;
        let (speed, rom_type) = split_byte(bytes[21]);
        if speed & !1 == 1 {
            score += VALID_SPEED_INDICATION
        }
        let is_fast = speed & 1 == 1;
        let rom_type = RomType::from_byte(rom_type)?;
        let (coprocessor, chips) = split_byte(bytes[22]);
        let rom_size = 0x400u32.wrapping_shl(bytes[23].into());
        let ram_size = 0x400u32.wrapping_shl(bytes[24].into());
        let ram_size = if ram_size == 0x400 { 0 } else { ram_size };
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
            (3..=5, 0, _) => Some(Coprocessor::Dsp),
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

    pub fn find_dsp_version(&self, rom_size: u32, ram_size: u32) -> Option<DspVersion> {
        let ver = match self.rom_type {
            RomType::LoRom => match (rom_size >> 20, ram_size >> 10) {
                (1, 0) => {
                    if self.name == "TOP GEAR 3000" {
                        DspVersion::Dsp4
                    } else {
                        DspVersion::Dsp1
                    }
                }
                (1, 32) => DspVersion::Dsp2,
                (1, 8) => DspVersion::Dsp3,
                (2, 8) => DspVersion::Dsp1,
                _ => DspVersion::Dsp1B,
            },
            RomType::HiRom => match (rom_size >> 20, ram_size >> 10) {
                (4, 0) => DspVersion::Dsp1,
                (4, 2) => DspVersion::Dsp1B, // TODO: Some games may use DSP1 (?)
                (2, 2 | 8) => DspVersion::Dsp1B,
                _ => DspVersion::Dsp1B, // TODO: is this appropriate?
            },
            _ => return None,
        };
        Some(ver)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountryFrameRate {
    Any,
    Ntsc,
    Pal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, InSaveState)]
pub struct Area {
    start: Addr24,
    end: Addr24,
}

impl Area {
    pub const fn new(start: Addr24, end: Addr24) -> Self {
        Self { start, end }
    }

    pub fn find(&self, addr: Addr24) -> bool {
        (self.start.bank..=self.end.bank).contains(&addr.bank)
            && (self.start.addr..=self.end.addr).contains(&addr.addr)
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(usize)]
enum ReadFunction {
    Rom = 0,
    Sram = 1,
    DspDr = 2,
    DspSr = 3,
}

type ReadFunPointer = fn(&mut Cartridge, u32) -> u8;

impl ReadFunction {
    pub fn get(&self) -> ReadFunPointer {
        const FUNS: [ReadFunPointer; 4] = [
            Cartridge::read_rom_mut,
            Cartridge::read_sram,
            Cartridge::read_dsp_data,
            Cartridge::read_dsp_status,
        ];
        FUNS[*self as usize]
    }
}

impl save_state::InSaveState for ReadFunction {
    fn serialize(&self, state: &mut SaveStateSerializer) {
        (*self as u8).serialize(state)
    }

    fn deserialize(&mut self, state: &mut SaveStateDeserializer) {
        let mut i: u8 = 0;
        i.deserialize(state);
        *self = match i {
            0 => Self::Rom,
            1 => Self::Sram,
            2 => Self::DspDr,
            3 => Self::DspSr,
            _ => panic!("unknown enum discriminant {}", i),
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(usize)]
enum WriteFunction {
    Ignore = 0,
    Sram = 1,
    DspDr = 2,
}

type WriteFunPointer = fn(&mut Cartridge, u32, u8);

impl WriteFunction {
    pub fn get(&self) -> WriteFunPointer {
        const FUNS: [WriteFunPointer; 3] = [
            Cartridge::ignore_write,
            Cartridge::write_sram,
            Cartridge::write_dsp_data,
        ];
        FUNS[*self as usize]
    }
}

impl save_state::InSaveState for WriteFunction {
    fn serialize(&self, state: &mut SaveStateSerializer) {
        (*self as u8).serialize(state)
    }

    fn deserialize(&mut self, state: &mut SaveStateDeserializer) {
        let mut i: u8 = 0;
        i.deserialize(state);
        *self = match i {
            0 => Self::Ignore,
            1 => Self::Sram,
            2 => Self::DspDr,
            _ => panic!("unknown enum discriminant {}", i),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, InSaveState)]
struct MapFunction {
    bank_mask: u8,
    bank_lshift: u8,
    addr_mask: u16,
}

impl MapFunction {
    pub fn run(&self, addr: Addr24) -> u32 {
        (u32::from(addr.bank & self.bank_mask) << self.bank_lshift)
            | u32::from(addr.addr & self.addr_mask)
    }
}

#[derive(Clone, InSaveState)]
pub struct MappingEntry {
    area: Area,
    map: MapFunction,
    read: ReadFunction,
    write: WriteFunction,
}

impl std::fmt::Debug for MappingEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Area as std::fmt::Debug>::fmt(&self.area, f)
    }
}

impl Default for MappingEntry {
    fn default() -> Self {
        Self {
            area: Area::new(Addr24::new(0, 0), Addr24::new(0, 0)),
            map: Default::default(),
            read: ReadFunction::Rom,
            write: WriteFunction::Ignore,
        }
    }
}

#[derive(Debug, Default, Clone, InSaveState)]
pub struct MemoryMapping {
    areas: Vec<MappingEntry>,
}

macro_rules! map {
    ($slf:ident @ $sb:literal:$sa:literal .. $eb:literal:$ea:literal => $r:ident | $w:ident [$bmask:literal << $bls:literal : $amask:literal]) => {
        $slf.areas.push(MappingEntry {
            area: Area::new(Addr24::new($sb, $sa), Addr24::new($eb, $ea)),
            map: MapFunction {
                bank_mask: $bmask,
                bank_lshift: $bls,
                addr_mask: $amask,
            },
            read: ReadFunction::$r,
            write: WriteFunction::$w,
        })
    };
}

impl MemoryMapping {
    pub fn find(&self, addr: Addr24) -> Option<(u32, &MappingEntry)> {
        self.areas.iter().find_map(|entry| {
            if entry.area.find(addr) {
                Some((entry.map.run(addr), entry))
            } else {
                None
            }
        })
    }
}

fn copy_rom(dst: &mut [u8], src: &[u8]) {
    if dst.len() <= src.len() {
        dst.copy_from_slice(&src[..dst.len()])
    } else if src.len().is_power_of_two() {
        for chunk in dst.chunks_mut(src.len()) {
            chunk.copy_from_slice(&src[..chunk.len()])
        }
    } else {
        let left_part = src.len().next_power_of_two() >> 1;
        dst[..left_part].copy_from_slice(&src[..left_part]);
        let dst_rest = dst.len() - left_part;
        let src_rest = src.len() - left_part;
        let right_part = src_rest.next_power_of_two();
        let count = dst_rest / right_part;
        let mut n = left_part;
        for _ in 0..count {
            copy_rom(&mut dst[n..n + right_part], &src[left_part..]);
            n += right_part;
        }
    }
}

fn create_rom(content: &[u8], size: u32) -> Vec<u8> {
    let size = size as usize;
    let mut rom = if content.len() > size {
        vec![0; content.len().next_power_of_two()]
    } else {
        vec![0; size]
    };
    copy_rom(&mut rom, content);
    rom
}

#[derive(Debug, Default, Clone, InSaveState)]
pub struct Cartridge {
    header: Header,
    rom: Vec<u8>,
    ram: Vec<u8>,
    dsp: Option<Dsp>,
    sa1: Option<Sa1>,
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

        let rom = create_rom(bytes, header.rom_size);

        use core::num::Wrapping;
        let Wrapping(checksum): Wrapping<u16> =
            rom.iter().copied().map(Into::into).map(Wrapping).sum();
        if checksum != header.checksum {
            eprintln!("warning: checksum did not match! Checksum in ROM is {:04x}; Calculated checksum is {:04x}", header.checksum, checksum);
        }

        let ram_size = header.ram_size;

        let dsp = if let Some(Coprocessor::Dsp) = header.coprocessor {
            let ver = header
                .find_dsp_version(rom.len() as u32, ram_size)
                .unwrap_or_else(|| panic!("could not select a NEC-DSP version for this game"));
            Some(Dsp::new(ver))
        } else {
            None
        };

        let sa1 = if let Some(Coprocessor::Sa1) = header.coprocessor {
            Some(Sa1::new())
        } else {
            None
        };

        let mut slf = Self {
            rom,
            ram: vec![0xff; ram_size as usize],
            mapping: MemoryMapping::default(),
            dsp,
            sa1,
            header,
        };

        slf.setup_memory_mappings();

        Ok(slf)
    }

    fn setup_memory_mappings(&mut self) {
        let map = &mut self.mapping;
        match self.header.rom_type {
            RomType::LoRom => {
                if let Some(dsp) = &self.dsp {
                    match (dsp.version(), self.rom.len() >> 20, self.ram.len() >> 10) {
                        (DspVersion::Dsp1 | DspVersion::Dsp1B | DspVersion::Dsp4, _, 0) => {
                            map!(map @ 0x30:0x8000 .. 0x3f:0xbfff => DspDr | DspDr [0xf<<14:0x3fff]);
                            map!(map @ 0x30:0xc000 .. 0x3f:0xffff => DspSr | Ignore [0xf<<14:0x3fff]);
                        }
                        (DspVersion::Dsp2 | DspVersion::Dsp3, 1, 8 | 32) => {
                            map!(map @ 0x20:0x8000 .. 0x3f:0xbfff => DspDr | DspDr [0x1f<<14:0x3fff]);
                            map!(map @ 0x20:0xc000 .. 0x3f:0xffff => DspSr | Ignore [0x1f<<14:0x3fff]);
                        }
                        (DspVersion::Dsp1 | DspVersion::Dsp1B, 2, 8) => {
                            map!(map @ 0x60:0x0000 .. 0x6f:0x3fff => DspDr | DspDr [0xf<<14:0x3fff]);
                            map!(map @ 0x60:0x4000 .. 0x6f:0x7fff => DspSr | Ignore [0xf<<14:0x3fff]);
                        }
                        _ => todo!("Could not guess any NEC-DSP memory mapping"),
                    }
                }
                map!(map @ 0x00:0x8000 .. 0x7d:0xffff => Rom | Ignore [0x7f<<15:0x7fff]);
                map!(map @ 0x80:0x8000 .. 0xff:0xffff => Rom | Ignore [0x7f<<15:0x7fff]);
                if self.ram.len() == 0 {
                    map!(map @ 0x40:0x0000 .. 0x7d:0x7fff => Rom | Ignore [0x7f<<15:0x7fff]);
                    map!(map @ 0xc0:0x0000 .. 0xff:0x7fff => Rom | Ignore [0x7f<<15:0x7fff]);
                } else {
                    map!(map @ 0x70:0x0000 .. 0x7d:0x7fff => Sram | Sram [0xf<<15:0xffff]);
                    map!(map @ 0xf0:0x0000 .. 0xff:0x7fff => Sram | Sram [0xf<<15:0xffff]);
                }
            }
            RomType::LoRomSA1 => (),
            RomType::HiRom => {
                map!(map @ 0x00:0x8000 .. 0x3f:0xffff => Rom | Ignore [0x3f<<16:0xffff]);
                map!(map @ 0x40:0x0000 .. 0x7d:0xffff => Rom | Ignore [0x3f<<16:0xffff]);
                map!(map @ 0x80:0x8000 .. 0xbf:0xffff => Rom | Ignore [0x3f<<16:0xffff]);
                map!(map @ 0xc0:0x0000 .. 0xff:0xffff => Rom | Ignore [0x3f<<16:0xffff]);
                if self.ram.len() > 0 {
                    map!(map @ 0x20:0x6000 .. 0x3f:0x7fff => Sram | Sram [0x3f<<13:0x1fff]);
                    map!(map @ 0xa0:0x6000 .. 0xbf:0x7fff => Sram | Sram [0x3f<<13:0x1fff]);
                }
                if let Some(dsp) = &self.dsp {
                    match dsp.version() {
                        DspVersion::Dsp1 => {
                            map!(map @ 0x00:0x6000 .. 0x1f:0x6fff => DspDr | DspDr [0<<0:0]);
                            map!(map @ 0x00:0x7000 .. 0x1f:0x7fff => DspSr | Ignore [0<<0:0]);
                            map!(map @ 0x80:0x6000 .. 0x9f:0x6fff => DspDr | DspDr [0<<0:0]);
                            map!(map @ 0x80:0x7000 .. 0x9f:0x7fff => DspSr | Ignore [0<<0:0]);
                        }
                        DspVersion::Dsp1B => {
                            map!(map @ 0x00:0x6000 .. 0x0f:0x6fff => DspDr | DspDr [0<<0:0]);
                            map!(map @ 0x00:0x7000 .. 0x0f:0x7fff => DspSr | Ignore [0<<0:0]);
                            map!(map @ 0x20:0x6000 .. 0x2f:0x6fff => DspDr | DspDr [0<<0:0]);
                            map!(map @ 0x20:0x7000 .. 0x2f:0x7fff => DspSr | Ignore [0<<0:0]);
                            map!(map @ 0x80:0x6000 .. 0x8f:0x6fff => DspDr | DspDr [0<<0:0]);
                            map!(map @ 0x80:0x7000 .. 0x8f:0x7fff => DspSr | Ignore [0<<0:0]);
                            map!(map @ 0xa0:0x6000 .. 0xaf:0x6fff => DspDr | DspDr [0<<0:0]);
                            map!(map @ 0xa0:0x7000 .. 0xaf:0x7fff => DspSr | Ignore [0<<0:0]);
                        }
                        ver => todo!("No HiRom memory mapping for {:?}", ver),
                    }
                }
            }
            ty => todo!("unsupported rom type {:?}", ty),
        }
    }

    pub fn read_byte(&mut self, addr: Addr24) -> Option<u8> {
        if self.has_sa1() {
            self.sa1_read::<false>(addr)
        } else {
            if let Some((index, MappingEntry { read, .. })) = self.mapping.find(addr) {
                Some(read.get()(self, index))
            } else {
                None
            }
        }
    }

    pub fn write_byte(&mut self, addr: Addr24, val: u8) {
        if self.has_sa1() {
            self.sa1_write::<false>(addr, val)
        } else {
            if let Some((index, MappingEntry { write, .. })) = self.mapping.find(addr) {
                write.get()(self, index, val)
            }
        }
    }

    pub const fn get_country_frame_rate(&self) -> CountryFrameRate {
        use CountryFrameRate::*;
        match self.header.country {
            0 | 1 | 13 | 15 => Ntsc,
            16 => Ntsc, // actually PAL-M
            2..=5 => Pal,
            6 => Pal, // actually SECAM
            7..=12 => Pal,
            17 => Pal,
            _ => Any,
        }
    }

    pub const fn header(&self) -> &Header {
        &self.header
    }

    pub fn title(&self) -> &str {
        &self.header.name
    }

    fn get_sram_addr(&self, addr: u32) -> usize {
        addr as usize & (self.ram.len() - 1)
    }

    fn get_rom_addr(&self, addr: u32) -> usize {
        addr as usize & (self.rom.len() - 1)
    }

    fn read_sram(&mut self, addr: u32) -> u8 {
        self.ram[self.get_sram_addr(addr)]
    }

    fn write_sram(&mut self, addr: u32, val: u8) {
        let addr = self.get_sram_addr(addr);
        self.ram[addr] = val
    }

    pub fn read_rom_mut(&mut self, addr: u32) -> u8 {
        self.read_rom(addr)
    }

    pub fn read_rom(&self, addr: u32) -> u8 {
        self.rom[self.get_rom_addr(addr)]
    }

    fn read_dsp_data(&mut self, _: u32) -> u8 {
        let dsp = self.dsp.as_mut().unwrap();
        dsp.refresh();
        dsp.read_dr()
    }

    fn write_dsp_data(&mut self, _: u32, val: u8) {
        let dsp = self.dsp.as_mut().unwrap();
        dsp.refresh();
        dsp.write_dr(val)
    }

    fn read_dsp_status(&mut self, _: u32) -> u8 {
        let dsp = self.dsp.as_mut().unwrap();
        dsp.refresh();
        dsp.read_sr()
    }

    fn ignore_write(&mut self, _addr: u32, _val: u8) {}

    /// Read from the cartridge
    pub fn read<D: Data>(&mut self, mut addr: Addr24) -> Option<D> {
        let mut arr: D::Arr = Default::default();
        let mut open_bus = None;
        for v in arr.as_mut() {
            *v = self.read_byte(addr).or(open_bus)?;
            open_bus = Some(*v);
            addr.addr = addr.addr.wrapping_add(1);
        }
        Some(D::from_bytes(&arr))
    }

    /// Write to the cartridge
    pub fn write<D: Data>(&mut self, mut addr: Addr24, value: D) {
        for &v in value.to_bytes().as_ref().iter() {
            self.write_byte(addr, v);
            addr.addr = addr.addr.wrapping_add(1);
        }
    }

    pub fn set_region(&mut self, pal: bool) {
        if let Some(dsp) = &mut self.dsp {
            dsp.set_timing_proportion(if pal {
                crate::timing::NECDSP_CPU_TIMING_PROPORTION_PAL
            } else {
                crate::timing::NECDSP_CPU_TIMING_PROPORTION_NTSC
            })
        }
        if let Some(sa1) = &mut self.sa1 {
            sa1.set_region(pal)
        }
    }

    pub fn tick(&mut self, n: Cycles) {
        if let Some(dsp) = &mut self.dsp {
            dsp.tick(n)
        }
    }

    pub fn refresh_coprocessors(&mut self) {
        if let Some(dsp) = &mut self.dsp {
            dsp.refresh()
        }
    }

    pub fn has_sa1(&self) -> bool {
        self.sa1.is_some()
    }

    pub fn sa1_ref(&self) -> &Sa1 {
        self.sa1
            .as_ref()
            .expect("unexpectedly queried sa1-chip in a non-sa1 cartridge")
    }

    pub fn sa1_mut(&mut self) -> &mut Sa1 {
        self.sa1
            .as_mut()
            .expect("unexpectedly queried sa1-chip in a non-sa1 cartridge")
    }
}
