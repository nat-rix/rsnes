use crate::oam::{CgRam, Oam, Object};
use core::mem::{replace, take};
use save_state::{SaveStateDeserializer, SaveStateSerializer};
use save_state_macro::*;

pub const VRAM_SIZE: usize = 0x8000;
pub const SCREEN_WIDTH: u32 = 256;
pub const MAX_SCREEN_HEIGHT: u32 = 224;
pub const MAX_SCREEN_HEIGHT_OVERSCAN: u32 = 239;
pub const CHIP_5C77_VERSION: u8 = 1;
pub const CHIP_5C78_VERSION: u8 = 3;

// TODO: Check the exact value of this.
// wiki.superfamicom.org/timing states that
// when we disable Force Blank mid-scanline,
// there is garbage for about 16-24 pixels.
pub const RAY_AHEAD_CYCLES: u16 = 20 * 4;

static OBJ_SIZES: [[[u8; 2]; 2]; 8] = [
    [[8, 8], [16, 16]],
    [[8, 8], [32, 32]],
    [[8, 8], [64, 64]],
    [[16, 16], [32, 32]],
    [[16, 16], [64, 64]],
    [[32, 32], [64, 64]],
    [[16, 32], [32, 64]],
    [[16, 32], [32, 32]],
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, InSaveState)]
pub struct RayPos {
    pub x: u16,
    pub y: u16,
}

impl RayPos {
    pub fn get<const N: u8>(&self) -> u16 {
        if N == 0 {
            self.x
        } else {
            self.y
        }
    }
}

#[derive(Debug, Default, Clone, Copy, InSaveState)]
pub struct LatchedState {
    pos: RayPos,
    flip: [bool; 2],
    latched: bool,
}

impl LatchedState {
    pub fn get<const N: u8>(&mut self, open_bus: u8) -> u8 {
        let c = self.pos.get::<N>();
        self.flip[N as usize] ^= true;
        if self.flip[N as usize] {
            (c & 0xff) as u8
        } else {
            ((c >> 8) & 1) as u8 | (open_bus & 0xfe)
        }
    }

    pub fn reset_flipflops(&mut self) -> bool {
        self.flip = [false; 2];
        take(&mut self.latched)
    }
}

#[derive(Debug, Clone, InSaveState)]
pub struct Vram {
    vram: [u16; VRAM_SIZE],
    unmapped_addr: u16,
    mapped_addr: u16,
    increment_first: bool,
    remap_mode: RemapMode,
    steps: u16,
    buffered: u16,
}

impl Vram {
    pub fn new() -> Self {
        Self {
            vram: [0; VRAM_SIZE],
            unmapped_addr: 0,
            mapped_addr: 0,
            increment_first: false,
            remap_mode: RemapMode::default(),
            steps: 1,
            buffered: 0,
        }
    }

    pub fn step(&mut self) {
        self.unmapped_addr = self.unmapped_addr.wrapping_add(self.steps);
        self.update_mapped();
    }

    pub fn update_mapped(&mut self) {
        self.mapped_addr = self.remap_mode.remap(self.unmapped_addr);
    }

    pub fn prefetch(&mut self) {
        self.buffered = self.read(self.mapped_addr)
    }

    pub fn get_mut(&mut self) -> &mut u16 {
        &mut self.vram[usize::from(self.mapped_addr) & (VRAM_SIZE - 1)]
    }

    pub fn read(&self, addr: u16) -> u16 {
        self.vram[usize::from(addr) & (VRAM_SIZE - 1)]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, InSaveState)]
pub struct Color {
    r: u8,
    g: u8,
    b: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const fn to_rgba8(self) -> [u8; 4] {
        [self.r, self.g, self.b, 255]
    }

    pub fn to_rgba8_with_brightness(self, brightness: u8) -> [u8; 4] {
        if brightness == 0 {
            [0; 4]
        } else {
            let b = u16::from(brightness.clamp(0, 15));
            self.map(|c| {
                let v = u16::from(c.clamp(0, 0x1f)) * b;
                ((v + (v << 4)) / 31) as u8
            })
            .to_rgba8()
        }
    }

    pub fn map<F: FnMut(u8) -> u8>(self, mut f: F) -> Self {
        Self {
            r: f(self.r),
            g: f(self.g),
            b: f(self.b),
        }
    }

    pub fn half(self) -> Self {
        self.map(|c| c >> 1)
    }
}

impl core::ops::Add for Color {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            r: self.r.wrapping_add(rhs.r),
            g: self.g.wrapping_add(rhs.g),
            b: self.b.wrapping_add(rhs.b),
        }
    }
}

impl core::ops::Sub for Color {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            r: self.r.saturating_sub(rhs.r),
            g: self.g.saturating_sub(rhs.g),
            b: self.b.saturating_sub(rhs.b),
        }
    }
}

impl From<u16> for Color {
    fn from(n: u16) -> Self {
        Self {
            r: (n & 0x1f) as u8,
            g: ((n >> 5) & 0x1f) as u8,
            b: ((n >> 10) & 0x1f) as u8,
        }
    }
}

#[derive(Debug, Clone, Copy, InSaveState)]
pub struct ColorMath {
    window: Window,
    backdrop: bool,
    half_color: bool,
    subtract_color: bool,
    add_subscreen: bool,
    behaviour: u8,
    color: Color,
}

impl ColorMath {
    pub const fn new() -> Self {
        Self {
            window: Window::new(),
            backdrop: false,
            half_color: false,
            subtract_color: false,
            add_subscreen: false,
            behaviour: 0,
            color: Color::new(0, 0, 0),
        }
    }
}

#[derive(Debug, Clone, Copy, InSaveState)]
pub struct Window {
    mask_logic: MaskLogic,
    windows: [bool; 2],
    window_inversion: [bool; 2],
}

impl Window {
    pub const fn new() -> Self {
        Self {
            mask_logic: MaskLogic::Or,
            windows: [false; 2],
            window_inversion: [false; 2],
        }
    }

    pub fn select(&mut self, val: u8) {
        self.window_inversion[0] = val & 1 > 0;
        self.windows[0] = val & 2 > 0;
        self.window_inversion[1] = val & 4 > 0;
        self.windows[1] = val & 8 > 0;
    }
}

#[derive(Debug, Clone, Copy, InSaveState)]
pub struct Layer {
    window: Window,
    main_screen: bool,
    sub_screen: bool,
    window_area_main_screen: bool,
    window_area_sub_screen: bool,
    color_math: bool,
}

impl Layer {
    pub const fn new() -> Self {
        Self {
            window: Window::new(),
            main_screen: false,
            sub_screen: false,
            window_area_main_screen: false,
            window_area_sub_screen: false,
            color_math: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MaskLogic {
    Or = 0,
    And = 1,
    Xor = 2,
    XNor = 3,
}

impl MaskLogic {
    pub const fn from_byte(val: u8) -> Self {
        match val & 3 {
            0 => Self::Or,
            1 => Self::And,
            2 => Self::Xor,
            3 => Self::XNor,
            _ => unreachable!(),
        }
    }

    pub const fn to_byte(self) -> u8 {
        self as u8
    }
}

impl save_state::InSaveState for MaskLogic {
    fn serialize(&self, state: &mut SaveStateSerializer) {
        self.to_byte().serialize(state)
    }

    fn deserialize(&mut self, state: &mut SaveStateDeserializer) {
        let mut n: u8 = 0;
        n.deserialize(state);
        *self = Self::from_byte(n)
    }
}

const fn sign_extend<const B: u16>(n: u16) -> u16 {
    if n & ((1 << B) >> 1) > 0 {
        n | !((1 << B) - 1)
    } else {
        n & (((1 << B) >> 1) - 1)
    }
}

/// Settings used to draw Mode 7's BG1.
#[derive(Debug, Clone, InSaveState)]
struct Mode7Settings {
    x_mirror: bool,
    y_mirror: bool,
    wrap: bool,
    fill: bool,
    prev_m7old: u8,
    // 13-bit signed
    offset: [u16; 2],
    // 13-bit signed
    center: [u16; 2],
    // 16-bit signed
    params: [u16; 4],

    // temporary scanline number
    tmpy: u8,
    // 11-bit signed
    tmp1: [u16; 2],

    tmp2: [i32; 2],
    tmp3: [i32; 2],
    tmp4: [i32; 2],
}

impl Mode7Settings {
    pub fn new() -> Self {
        Self {
            x_mirror: false,
            y_mirror: false,
            wrap: true,
            fill: false,
            prev_m7old: 0,
            offset: [0; 2],
            center: [0; 2],
            params: [0; 4],

            tmpy: 0,
            tmp1: [0; 2],
            tmp2: [0; 2],
            tmp3: [0; 2],
            tmp4: [0; 2],
        }
    }

    pub fn write_m7old(&mut self, val: u8) -> u16 {
        u16::from(replace(&mut self.prev_m7old, val)) | (u16::from(val) << 8)
    }

    // called when offset[C] or center[C] updated
    fn update_tmp1<const C: usize>(&mut self) {
        let val = self.offset[C].wrapping_sub(self.center[C]);
        self.tmp1[C] = if val & 0x2000 > 0 {
            val | 0xfc00
        } else {
            val & 0x3ff
        };
        self.update_tmp2::<0>();
        self.update_tmp2::<1>();
    }

    // called when tmp1[*] or params[C * 2] or params[C * 2 + 1] updated
    fn update_tmp2<const C: usize>(&mut self) {
        let a = self.params[C << 1] as i16 as i32 * (self.tmp1[0] as i16 as i32);
        let b = self.params[(C << 1) | 1] as i16 as i32 * (self.tmp1[1] as i16 as i32);
        let a = (a as u32 & !0x3f) as i32;
        let b = (b as u32 & !0x3f) as i32;
        self.tmp2[C] = a + b + (i32::from(self.center[C] as i16) << 8);
        self.update_tmp4::<C>();
    }

    // called when params[C * 2 + 1] or tmpy updated
    fn update_tmp3<const C: usize>(&mut self) {
        let val = self.params[(C << 1) | 1] as i16 as i32 * i32::from(self.tmpy);
        self.tmp3[C] = (val as u32 & !0x3f) as i32;
        self.update_tmp4::<C>();
    }

    // called when tmp2[C] or tmp3[C] updated
    fn update_tmp4<const C: usize>(&mut self) {
        self.tmp4[C] = self.tmp2[C].wrapping_add(self.tmp3[C])
    }

    fn update_param(&mut self, id: u8, val: u16) {
        self.params[usize::from(id)] = val;
        if id < 2 {
            self.update_tmp2::<0>();
            if id == 1 {
                self.update_tmp3::<0>();
            }
        } else {
            self.update_tmp2::<1>();
            if id == 3 {
                self.update_tmp3::<1>();
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, InSaveState)]
struct CachedTile {
    tile: u64,
    palette_nr: u8,
    x: u8,
    prio: bool,
}

#[derive(Debug, Clone, Copy, InSaveState)]
pub struct Bg {
    layer: Layer,
    mosaic: bool,
    mosaic_start: Option<u16>,
    tile_size: [u8; 2],
    map_base_addr: u16,
    tile_base_addr: u16,
    size: [u8; 2],
    scroll: [u16; 2],
    scroll_prev: [u8; 2],

    cached_tile: Option<CachedTile>,
}

#[derive(Debug, Clone, Copy, InSaveState)]
pub struct ObjCacheEntry {
    palette_addr: u8,
    prio: u8,
}

impl ObjCacheEntry {
    const EMPTY: Self = Self {
        palette_addr: 0,
        prio: 0xff,
    };

    pub fn write(&mut self, val: Self) {
        if self.prio == 0xff || val.prio > self.prio {
            *self = val
        }
    }
}

impl Bg {
    pub const fn new() -> Self {
        Self {
            layer: Layer::new(),
            mosaic: false,
            mosaic_start: None,
            tile_size: [8, 8],
            map_base_addr: 0,
            tile_base_addr: 0,
            size: [32, 32],
            scroll: [0; 2],
            scroll_prev: [0; 2],

            cached_tile: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum DrawLayer {
    Bg { nr: u8, bits: u8, prio: bool },
    Sprite { prio: u8 },
}

impl save_state::InSaveState for DrawLayer {
    fn serialize(&self, state: &mut SaveStateSerializer) {
        match self {
            Self::Bg { nr, bits, prio } => {
                true.serialize(state);
                nr.serialize(state);
                bits.serialize(state);
                prio.serialize(state);
            }
            Self::Sprite { prio } => {
                false.serialize(state);
                prio.serialize(state);
            }
        }
    }

    fn deserialize(&mut self, state: &mut SaveStateDeserializer) {
        let mut i: bool = false;
        i.deserialize(state);
        *self = if i {
            let (mut nr, mut bits, mut prio) = (0, 0, false);
            nr.deserialize(state);
            bits.deserialize(state);
            prio.deserialize(state);
            Self::Bg { nr, bits, prio }
        } else {
            let mut prio = 0;
            prio.deserialize(state);
            Self::Sprite { prio }
        }
    }
}

#[derive(Debug, Clone, InSaveState)]
struct Layers {
    arr: [DrawLayer; 12],
    size: u8,
}

impl Layers {
    pub const fn from_bgmode(bg_mode: BgMode) -> Self {
        macro_rules! to_list {
            ($($x:expr),+ $(,)?) => {{
                let mut arr = [DrawLayer::Sprite { prio: 0 }; 12];
                let mut size = 0;
                $({ let i = $x; arr[size as usize] = i ; size += 1; })+
                Layers { arr, size }
            }};
        }
        macro_rules! BG { (nr $i:literal, colors $b:expr $(, $p:ident)*) => {
            DrawLayer::Bg {
                nr: $i - 1,
                bits: { let a: u16 = $b; a }.trailing_zeros() as u8,
                prio: false $(|| { let $p = true; $p })*,
            }
        }; }
        macro_rules! S {
            ($p:literal) => {
                DrawLayer::Sprite { prio: $p }
            };
        }
        match (bg_mode.num, bg_mode.bg3_prio, bg_mode.extbg) {
            (0, _, _) => to_list![
                S!(3),
                BG!(nr 1, colors 4, prio),
                BG!(nr 2, colors 4, prio),
                S!(2),
                BG!(nr 1, colors 4),
                BG!(nr 2, colors 4),
                S!(1),
                BG!(nr 1, colors 4),
                BG!(nr 2, colors 4),
                S!(0),
                BG!(nr 1, colors 4),
                BG!(nr 2, colors 4),
            ],
            (1, false, _) => to_list![
                S!(3),
                BG!(nr 1, colors 16, prio),
                BG!(nr 2, colors 16, prio),
                S!(2),
                BG!(nr 1, colors 16),
                BG!(nr 2, colors 16),
                S!(1),
                BG!(nr 3, colors 4, prio),
                S!(0),
                BG!(nr 3, colors 4),
            ],
            (1, true, _) => to_list![
                BG!(nr 3, colors 4, prio),
                S!(3),
                BG!(nr 1, colors 16, prio),
                BG!(nr 2, colors 16, prio),
                S!(2),
                BG!(nr 1, colors 16),
                BG!(nr 2, colors 16),
                S!(1),
                S!(0),
                BG!(nr 3, colors 4),
            ],
            (2..=5, _, _) => {
                let (c1, c2) = match bg_mode.num {
                    2 => (16, 16),
                    3 => (256, 16),
                    4 => (256, 4),
                    _ => (16, 4),
                };
                to_list![
                    S!(3),
                    BG!(nr 1, colors c1, prio),
                    S!(2),
                    BG!(nr 2, colors c2, prio),
                    S!(1),
                    BG!(nr 1, colors c1),
                    S!(0),
                    BG!(nr 2, colors c2),
                ]
            }
            (6, _, _) => to_list![
                S!(3),
                BG!(nr 1, colors 16, prio),
                S!(2),
                S!(1),
                BG!(nr 1, colors 16),
                S!(0),
            ],
            (7, _, false) => to_list![S!(3), S!(2), S!(1), BG!(nr 1, colors 256), S!(0),],
            (7, _, true) => to_list![
                S!(3),
                S!(2),
                BG!(nr 2, colors 128, prio),
                S!(1),
                BG!(nr 1, colors 256),
                S!(0),
                BG!(nr 2, colors 128),
            ],
            _ => todo!(),
        }
    }
}

#[derive(Debug, Clone, Copy, InSaveState)]
pub struct BgMode {
    num: u8,
    bg3_prio: bool,
    extbg: bool,
}

impl BgMode {
    const fn new(num: u8, bg3_prio: bool, extbg: bool) -> Self {
        Self {
            num,
            bg3_prio,
            extbg,
        }
    }
}

#[derive(Debug, Clone, InSaveState)]
pub struct Ppu<FB: crate::backend::FrameBuffer> {
    #[except((|_v, _s| ()), (|_v, _s| ()))]
    pub frame_buffer: FB,
    oam: Oam,
    cgram: CgRam,
    vram: Vram,
    bgs: [Bg; 4],
    bg_mode: BgMode,
    bg3_prio: bool,
    pos: RayPos,
    latched: LatchedState,
    brightness: u8,
    draw_layers: Layers,
    /// Object sizes in (x, y) = `[u8; 2]` for small sprites (=`obj_size[0]`) and large sprites
    /// (=`obj_size[1]`) in pixels
    obj_size: [[u8; 2]; 2],
    /// Tile base address without and with gap
    obj_tile_addr: [u16; 2],
    obj_layer: Layer,
    obj_cache: [ObjCacheEntry; 256],
    overflow_flags: u8,
    color_math: ColorMath,
    direct_color_mode: bool,
    object_interlace: bool,
    interlace_active: bool,
    window_positions: [[u8; 2]; 2],
    overscan: bool,
    pseudo512: bool,
    mosaic_size: u8,
    mode7_settings: Mode7Settings,
    field: bool,
    force_blank: bool,
    is_pal: bool,
    pub(crate) open_bus1: u8,
    pub(crate) open_bus2: u8,
}

impl<FB: crate::backend::FrameBuffer> Ppu<FB> {
    pub fn new(frame_buffer: FB, is_pal: bool) -> Self {
        let bg_mode = BgMode::new(0, false, false);
        Self {
            frame_buffer,
            oam: Oam::new(),
            cgram: CgRam::new(),
            vram: Vram::new(),
            bgs: [Bg::new(); 4],
            bg_mode,
            bg3_prio: false,
            pos: Default::default(),
            latched: Default::default(),
            brightness: 15,
            draw_layers: Layers::from_bgmode(bg_mode),
            obj_size: OBJ_SIZES[0],
            obj_tile_addr: [0; 2],
            obj_layer: Layer::new(),
            obj_cache: [ObjCacheEntry::EMPTY; 256],
            overflow_flags: 0,
            color_math: ColorMath::new(),
            direct_color_mode: false,
            object_interlace: false,
            interlace_active: false,
            window_positions: [[0; 2]; 2],
            overscan: false,
            pseudo512: false,
            mosaic_size: 0,
            mode7_settings: Mode7Settings::new(),
            field: false,
            force_blank: true,
            is_pal,
            open_bus1: 0,
            open_bus2: 0,
        }
    }

    /// 2134 - 213f
    pub fn read_register(&mut self, addr: u8) -> Option<u8> {
        assert!(addr >= 0x34 && addr <= 0x3f);
        match addr {
            0x34..=0x36 => {
                // MPYL/M/H
                let x = self.mode7_settings.params[0] as i16 as i32;
                let y = (self.mode7_settings.params[1] >> 8) as i8 as i32;
                Some(((x * y) as u32).to_le_bytes()[usize::from(addr & 3)])
            }
            0x37 => {
                // SLHV - Software Latch for H/V Counter
                self.latch();
                None
            }
            0x38 => Some(self.oam.read()), // RDOAM
            0x39 | 0x3a => {
                // RDVRAML/H
                let is_second = addr == 0x3a;
                let val = self.vram.buffered.to_le_bytes()[usize::from(is_second)];
                if self.vram.increment_first ^ is_second {
                    self.vram.prefetch();
                    self.vram.step();
                }
                Some(val)
            }
            0x3b => Some(self.cgram.read(self.open_bus2)), // RDCGRAM
            0x3c => Some(self.latched.get::<0>(self.open_bus2)), // OPHCT
            0x3d => Some(self.latched.get::<1>(self.open_bus2)), // OPHCT
            0x3e => Some(self.overflow_flags | (self.open_bus1 & 0x10) | CHIP_5C77_VERSION), // STAT77
            0x3f => {
                // STAT78
                self.latched.flip = [false; 2];
                Some(
                    ((take(&mut self.latched.latched) as u8) << 6)
                        | (self.open_bus2 & 0x20)
                        | CHIP_5C78_VERSION
                        | ((self.is_pal as u8) << 4),
                )
            }
            _ => unreachable!(),
        }
    }

    /// 2100 - 2133
    pub fn write_register(&mut self, addr: u8, val: u8) {
        assert!(addr <= 0x33);
        match addr {
            0x00 => {
                // INIDISP
                self.force_blank = val & 0x80 > 0;
                self.brightness = val & 15;
            }
            0x01 => {
                // OBSEL
                self.obj_size = OBJ_SIZES[usize::from(val >> 5)];
                let addr = u16::from(val & 7) << 13;
                let gap = u16::from(val & 0x18) << 9;
                self.obj_tile_addr = [addr, addr.wrapping_add(gap).wrapping_add(0x1000)];
            }
            0x02 => self.oam.set_addr_low(val),  // OAMADDL
            0x03 => self.oam.set_addr_high(val), // OAMADDH
            0x04 => self.oam.write(val),         // OAMDATA
            0x05 => {
                // BGMODE
                self.bg_mode.num = val & 7;
                self.bg_mode.bg3_prio = val & 8 > 0;
                self.draw_layers = Layers::from_bgmode(self.bg_mode);
                let val = val >> 4;
                for i in 0u8..4 {
                    let bg = &mut self.bgs[usize::from(i)];
                    bg.tile_size = [8 << ((val >> i) & 1); 2];
                    match self.bg_mode.num {
                        5 => bg.tile_size[0] = 16,
                        6 => bg.tile_size = [16, 8],
                        7 => bg.tile_size = [8, 8],
                        _ => (),
                    }
                }
            }
            0x06 => {
                // MOSAIC
                self.mosaic_size = (val >> 4) + 1;
                for i in 0u8..4 {
                    self.bgs[usize::from(i)].mosaic = (val >> i) & 1 > 0;
                }
            }
            0x07..=0x0a => {
                // BGnSC
                let bg_nr = (addr + 1) & 3;
                let bg = &mut self.bgs[usize::from(bg_nr)];
                bg.map_base_addr = u16::from(val & 0xfc) << 8;
                bg.size = [32 << (val & 1), 32 << ((val >> 1) & 1)];
            }
            0x0b | 0x0c => {
                // BGnnNBA
                let bg_nr = usize::from(!addr & 2);
                self.bgs[bg_nr].tile_base_addr = u16::from(val & 15) << 12;
                self.bgs[bg_nr | 1].tile_base_addr = u16::from(val & 0xf0) << 8;
            }
            0x0d..=0x14 => {
                // M7xOFS and BGnxOFS
                if (0x0d..=0x0e).contains(&addr) {
                    let val = sign_extend::<13>(self.mode7_settings.write_m7old(val));
                    if addr == 0x0d {
                        self.mode7_settings.offset[0] = val;
                        self.mode7_settings.update_tmp1::<0>();
                    } else {
                        self.mode7_settings.offset[1] = val;
                        self.mode7_settings.update_tmp1::<1>();
                    }
                }
                let bg = &mut self.bgs[usize::from(((addr - 5) >> 1) & 3)];
                let old = replace(&mut bg.scroll_prev[0], val);
                bg.scroll[usize::from(!addr & 1)] = (u16::from(val) << 8)
                    | u16::from(if addr & 1 > 0 {
                        (old & 0xf8) | replace(&mut bg.scroll_prev[1], val) & 7
                    } else {
                        old
                    });
            }
            0x15 => {
                // VMAIN
                self.vram.increment_first = val & 0x80 == 0;
                self.vram.remap_mode = RemapMode::new(val >> 2);
                self.vram.steps = match val & 3 {
                    0 => 1,
                    1 => 32,
                    _ => 128,
                };
            }
            0x16 | 0x17 => {
                // VMADDL
                let mut bytes = self.vram.unmapped_addr.to_le_bytes();
                bytes[usize::from(addr & 1)] = val;
                self.vram.unmapped_addr = u16::from_le_bytes(bytes);
                self.vram.update_mapped();
                self.vram.prefetch();
            }
            0x18 | 0x19 => {
                // VMDATAx
                let word = self.vram.get_mut();
                let mut bytes = word.to_le_bytes();
                bytes[usize::from(addr & 1)] = val;
                *word = u16::from_le_bytes(bytes);
                if (addr & 1 > 0) ^ self.vram.increment_first {
                    self.vram.step()
                }
            }
            0x1a => {
                // M7SEL
                self.mode7_settings.x_mirror = val & 1 > 0;
                self.mode7_settings.y_mirror = val & 2 > 0;
                self.mode7_settings.wrap = val & 0x80 == 0;
                self.mode7_settings.fill = val & 0x40 > 0;
            }
            0x1b..=0x1e => {
                // M7A-M7D
                let id = (addr + 1) & 3;
                let val = self.mode7_settings.write_m7old(val);
                self.mode7_settings.update_param(id, val);
            }
            0x1f | 0x20 => {
                // M7X/Y
                let val = sign_extend::<13>(self.mode7_settings.write_m7old(val));
                if addr == 0x1f {
                    self.mode7_settings.center[0] = val;
                    self.mode7_settings.update_tmp1::<0>();
                } else {
                    self.mode7_settings.center[1] = val;
                    self.mode7_settings.update_tmp1::<1>();
                }
            }
            0x21 => self.cgram.set_addr(val), // CGADD
            0x22 => self.cgram.write(val),    // CGDATA
            0x23..=0x25 => {
                // WnnSEL
                let (w1, w2) = match addr {
                    0x23 => {
                        let [bg0, bg1, ..] = &mut self.bgs;
                        (&mut bg0.layer.window, &mut bg1.layer.window)
                    }
                    0x24 => {
                        let [.., bg2, bg3] = &mut self.bgs;
                        (&mut bg2.layer.window, &mut bg3.layer.window)
                    }
                    0x25 => (&mut self.obj_layer.window, &mut self.color_math.window),
                    _ => unreachable!(),
                };
                w1.select(val);
                w2.select(val >> 4);
            }
            0x26..=0x29 => {
                // WHn
                self.window_positions[usize::from((!addr & 2) >> 1)][usize::from(addr & 1)] = val
            }
            0x2a => {
                // WBGLOG
                let mut val = val;
                for i in 0..4 {
                    self.bgs[i].layer.window.mask_logic = MaskLogic::from_byte(val);
                    val >>= 2;
                }
            }
            0x2b => {
                // WOBJLOG
                self.obj_layer.window.mask_logic = MaskLogic::from_byte(val);
                self.color_math.window.mask_logic = MaskLogic::from_byte(val >> 2);
            }
            0x2c | 0x2d => {
                // TM / TS
                let mut val = val;
                for layer in self.layers_mut() {
                    if addr == 0x2c {
                        layer.main_screen = val & 1 != 0
                    } else {
                        layer.sub_screen = val & 1 != 0
                    }
                    val >>= 1;
                }
            }
            0x2e | 0x2f => {
                // TMW / TSW
                let mut val = val;
                for layer in self.layers_mut() {
                    if addr == 0x2e {
                        layer.window_area_main_screen = val & 1 > 0
                    } else {
                        layer.window_area_sub_screen = val & 1 > 0
                    }
                    val >>= 1;
                }
            }
            0x30 => {
                // CGWSEL
                self.direct_color_mode = val & 1 > 0;
                self.color_math.add_subscreen = val & 2 > 0;
                self.color_math.behaviour = val >> 4;
            }
            0x31 => {
                // CGADSUB
                let mut val = val;
                for i in 0..4 {
                    self.bgs[i].layer.color_math = val & 1 > 0;
                    val >>= 1;
                }
                self.obj_layer.color_math = val & 1 > 0;
                self.color_math.backdrop = val & 2 > 0;
                self.color_math.half_color = val & 4 > 0;
                self.color_math.subtract_color = val & 8 > 0;
            }
            0x32 => {
                // COLDATA
                let component = val & 0x1f;
                if val & 0x20 > 0 {
                    self.color_math.color.r = component
                }
                if val & 0x40 > 0 {
                    self.color_math.color.g = component
                }
                if val & 0x80 > 0 {
                    self.color_math.color.b = component
                }
            }
            0x33 => {
                // SETINI
                self.interlace_active = val & 1 > 0;
                self.object_interlace = val & 2 > 0;
                self.overscan = val & 4 > 0;
                self.pseudo512 = val & 8 > 0;
                self.bg_mode.extbg = val & 0x40 > 0;
                self.draw_layers = Layers::from_bgmode(self.bg_mode);
                if val & 0x80 > 0 {
                    todo!("what the hack is super imposing!?")
                }
            }
            _ => unreachable!(),
        }
    }

    fn get_layer_from_draw_layer(&self, layer: &DrawLayer) -> &Layer {
        match layer {
            DrawLayer::Bg { nr, .. } => &self.bgs[usize::from(*nr)].layer,
            DrawLayer::Sprite { .. } => &self.obj_layer,
        }
    }

    pub fn fetch_tile_by_nr(
        &mut self,
        y: u16,
        tile_base: u16,
        tile_nr: u16,
        xflip: bool,
        planes: u8,
    ) -> u64 {
        let addr = tile_base
            .wrapping_add(tile_nr << (2 + planes.trailing_zeros()))
            .wrapping_add(y & 7);
        let mut tile = 0;
        for i in 0..planes >> 1 {
            let mut plane = self.vram.read(addr.wrapping_add(u16::from(i) << 3));
            if xflip {
                plane = u16::from_le_bytes(plane.to_le_bytes().map(u8::reverse_bits));
            }
            tile |= u64::from(plane) << (i << 4)
        }
        tile
    }

    pub fn fetch_tile(
        &mut self,
        x: u16,
        y: u16,
        tile_base: u16,
        tile_w: u8,
        tile_h: u8,
        char_nr: u16,
        xflip: bool,
        planes: u8,
    ) -> u64 {
        let [tile_x, tile_y] = [
            (x & 0xff) as u8 & (tile_w - 1),
            (y & 0xff) as u8 & (tile_h - 1),
        ];
        let tile_nr = char_nr
            .wrapping_add(u16::from(tile_x >> 3))
            .wrapping_add(u16::from(tile_y >> 3) << 4);
        self.fetch_tile_by_nr(y, tile_base, tile_nr, xflip, planes)
    }

    fn decode_tile(tile: u64, x: u16) -> u8 {
        let dx = ((x ^ 7) & 7) as u8;
        let mut color = 0;
        for (i, b) in ((tile >> dx) & 0x01_01_01_01_01_01_01_01)
            .to_le_bytes()
            .iter()
            .enumerate()
        {
            color |= b << i
        }
        color
    }

    fn fetch_bg7_tile(&mut self, x: u8, nr: u8, prio: bool) -> Option<Color> {
        let x = if self.mode7_settings.x_mirror { !x } else { x };

        let v = [
            (self.mode7_settings.tmp4[0], self.mode7_settings.params[0]),
            (self.mode7_settings.tmp4[1], self.mode7_settings.params[2]),
        ]
        .map(|(c, p)| c.wrapping_add(p as i16 as i32 * i32::from(x)));

        let v = v.map(|c| (((c as u32) >> 8) & 0xffff) as u16);
        let tile_nr = if self.mode7_settings.wrap || !v.iter().any(|&c| c > 0x3ff) {
            let tile_nrs = v.map(|c| (c >> 3) & 0x7f);
            tile_nrs[0] + (tile_nrs[1] << 7)
        } else if self.mode7_settings.fill {
            0
        } else {
            return None;
        };
        let char_nr = self.vram.read(tile_nr).to_le_bytes()[0];
        let char_addr = u16::from(char_nr) << 6;
        let pixel_addr = char_addr
            .wrapping_add(v[0] & 7)
            .wrapping_add((v[1] & 7) << 3);
        let cgram_addr = self.vram.read(pixel_addr).to_le_bytes()[1];
        if cgram_addr == 0 || (nr == 1 && (cgram_addr & 0x80 == 0) == prio) {
            None
        } else {
            Some(if self.direct_color_mode {
                Color {
                    r: (cgram_addr & 7) << 2,
                    g: (cgram_addr & 0x38) >> 1,
                    b: (cgram_addr & 0xc0) >> 3,
                }
            } else {
                self.cgram.read16(cgram_addr).into()
            })
        }
    }

    pub fn fetch_bg_tile(&mut self, x: u8, y: u16, nr: u8, bits: u8, prio: bool) -> Option<Color> {
        if self.bg_mode.num == 7 {
            return self.fetch_bg7_tile(x, nr, prio);
        }
        // TODO: implement offset-per-tile
        let bg = &self.bgs[usize::from(nr)];
        let x = (x as i16 + (((bg.scroll[0] << 6) as i16) >> 6)) as u16 & 0x3ff;
        let y = (y as i16 + (((bg.scroll[1] << 6) as i16) >> 6)) as u16 & 0x3ff;
        let (x, y) = if let Some(start) = bg.mosaic_start {
            let sz = self.mosaic_size as u16;
            let ys = y - start;
            (x - (x % sz), (ys - (ys % sz)) + start)
        } else {
            (x, y)
        };
        let cache_x = (x >> 3) as u8;
        let tile = if let Some(tile) = bg.cached_tile.filter(|t| t.x == cache_x) {
            tile
        } else {
            let tile_x = (x >> bg.tile_size[0].trailing_zeros()) & 0x3f;
            let tile_y = (y >> bg.tile_size[1].trailing_zeros()) & 0x3f;
            let map_nr = match bg.size {
                [64, 32] => (tile_x << 5) & 0x400,
                [32, 64] => (tile_y << 5) & 0x400,
                [64, 64] => ((tile_x << 5) | ((tile_y & 0x20) << 6)) & 0xc00,
                _ => 0,
            };
            let map_addr = bg
                .map_base_addr
                .wrapping_add((tile_x & 0x1f) | ((tile_y & 0x1f) << 5))
                .wrapping_add(map_nr);
            let map_val = self.vram.read(map_addr);
            let (char_nr, palette_nr, sel_prio, xflip, yflip) = (
                map_val & 0x3ff,
                ((map_val >> 10) & 7) as u8,
                map_val & 0x2000 > 0,
                map_val & 0x4000 > 0,
                map_val & 0x8000 > 0,
            );
            if sel_prio ^ prio {
                self.bgs[usize::from(nr)].cached_tile = None;
                return None;
            }
            let x = if xflip { !x } else { x };
            let y = if yflip { !y } else { y };
            let (base, tw, th) = (bg.tile_base_addr, bg.tile_size[0], bg.tile_size[1]);
            let tile = self.fetch_tile(x, y, base, tw, th, char_nr, xflip, bits);
            let tile = CachedTile {
                x: cache_x,
                prio: sel_prio,
                tile,
                palette_nr,
            };
            self.bgs[usize::from(nr)].cached_tile = Some(tile);
            tile
        };
        if tile.prio ^ prio {
            return None;
        }
        let palette_idx = Self::decode_tile(tile.tile, x);
        if palette_idx == 0 {
            return None;
        }
        let color = if self.direct_color_mode && bits == 8 {
            Color {
                r: ((palette_idx & 7) << 2) | ((tile.palette_nr & 1) << 1),
                g: ((palette_idx & 0x38) >> 1) | (tile.palette_nr & 2),
                b: ((palette_idx & 0xc0) >> 3) | (tile.palette_nr & 4),
            }
        } else {
            let cg_addr = if self.bg_mode.num == 0 {
                (tile.palette_nr << 2) | palette_idx | (nr << 5) as u8
            } else {
                (tile.palette_nr << bits) | palette_idx
            };
            self.cgram.read16(cg_addr).into()
        };
        Some(color)
    }

    pub fn fetch_screen(
        &mut self,
        x: u8,
        y: u16,
        mainscreen: bool,
        subscreen: bool,
    ) -> ([Color; 2], bool) {
        let [mut main_found, mut sub_found] = [false; 2];
        let [mut main, mut sub] = [Color::new(0, 0, 0), self.color_math.color];
        let mut layer_color_math = None;
        for draw_ly_idx in 0..self.draw_layers.size {
            let draw_ly = &self.draw_layers.arr[usize::from(draw_ly_idx)];
            let ly = self.get_layer_from_draw_layer(&draw_ly);
            let in_window = self.is_in_window(x, &ly.window);
            let [is_main, is_sub] = [
                ly.main_screen
                    && !main_found
                    && mainscreen
                    && (!ly.window_area_main_screen || !in_window),
                ly.sub_screen
                    && !sub_found
                    && subscreen
                    && (!ly.window_area_sub_screen || !in_window),
            ];
            if !is_main && !is_sub {
                continue;
            }
            let mut layer_color_math_ = ly.color_math;
            if let Some(color) = match draw_ly {
                &DrawLayer::Bg { nr, bits, prio } => self.fetch_bg_tile(x, y, nr, bits, prio),
                &DrawLayer::Sprite { prio } => {
                    let entry = self.obj_cache[usize::from(x)];
                    if prio == entry.prio && entry.palette_addr != 0 {
                        layer_color_math_ &= entry.palette_addr & 0x40 > 0;
                        Some(self.cgram.read16(entry.palette_addr).into())
                    } else {
                        None
                    }
                }
            } {
                if is_main {
                    main_found = true;
                    main = color;
                    layer_color_math = Some(layer_color_math_);
                    if sub_found || !subscreen {
                        break;
                    }
                }
                if is_sub {
                    sub_found = true;
                    sub = color;
                    if main_found || !mainscreen {
                        break;
                    }
                }
            }
        }
        if !main_found && mainscreen {
            main = self.cgram.main_screen_backdrop().into()
        }
        (
            [main, sub],
            layer_color_math.unwrap_or_else(|| self.color_math.backdrop),
        )
    }

    pub fn draw_pixel(&mut self, x: u8, y: u16) -> [u8; 4] {
        let mut lazy_in_window = None;
        let mut in_window = || {
            if let Some(iw) = lazy_in_window {
                iw
            } else {
                let val = self.is_in_window(x, &self.color_math.window);
                lazy_in_window = Some(val);
                val
            }
        };
        let [main_enable, color_enable] = [
            self.color_math.behaviour >> 2,
            self.color_math.behaviour & 3,
        ]
        .map(|i| match i {
            0 | 3 => i == 0,
            _ => (i == 2) ^ in_window(),
        });
        let ([main, sub], color_math) = self.fetch_screen(
            x,
            y,
            main_enable,
            color_enable && self.color_math.add_subscreen,
        );
        let color = if color_math && color_enable {
            let mut color = if self.color_math.subtract_color {
                main - sub
            } else {
                main + sub
            };
            if self.color_math.half_color && main_enable {
                color = color.half();
            }
            color.map(|c| c.clamp(0, 0x1f))
        } else {
            main
        };
        color.to_rgba8_with_brightness(self.brightness)
    }

    fn draw_obj_8x8_tile(&mut self, obj: &Object, row: u8, tile_x: u8, tile_y: u8, size: [u8; 2]) {
        let base = self.obj_tile_addr[usize::from(obj.attrs & 1)];
        let xflip = obj.is_xflip();

        let palette_nr = obj.get_palette_nr();
        let prio = obj.get_priority();
        let tile_addr = obj.get_tile_addr(base, tile_x, tile_y);
        let tile = self.fetch_tile_by_nr(row.into(), tile_addr, 0, false, 4);
        for x in 0u8..8 {
            let off = i16::from(x).wrapping_add(i16::from(tile_x) << 3);
            let gx = (if xflip {
                i16::from(size[0]) - off - 1
            } else {
                off
            })
            .wrapping_add(obj.x);
            if (0..=255).contains(&gx) {
                let palette_idx = Self::decode_tile(tile, x.into());
                if palette_idx > 0 {
                    self.obj_cache[gx as usize].write(ObjCacheEntry {
                        palette_addr: 0x80 | (palette_nr << 4) | palette_idx,
                        prio,
                    });
                };
            }
        }
    }

    fn refill_obj_cache(&mut self, y: u16) {
        self.obj_cache.fill(ObjCacheEntry::EMPTY);
        let y = (y & 0xff) as u8;
        let mut objs_in_line = 0;
        let mut tiles_in_line = 0;
        'obj_loop: for obj in self.oam.objs {
            let size = self.obj_size[usize::from(obj.is_large)];
            if (-i16::from(size[0]) >= obj.x && obj.x != -256)
                || obj.x >= 256
                || !((obj.y..=obj.y.saturating_add(size[1] - 1)).contains(&y)
                    || i16::from(y) < i16::from(obj.y) + i16::from(size[1]) - 256)
            {
                continue;
            }
            if objs_in_line >= 32 {
                self.overflow_flags |= 0x40;
                break 'obj_loop;
            }
            objs_in_line += 1;
            let y = y.wrapping_sub(obj.y);
            let y = if obj.is_yflip() { size[1] - y - 1 } else { y };
            for tile_id in 0..size[0] >> 3 {
                let left = obj.x + i16::from(tile_id << 3);
                if left < -7 || left >= 256 {
                    continue;
                }
                if tiles_in_line >= 34 {
                    self.overflow_flags |= 0x80;
                    break 'obj_loop;
                }
                tiles_in_line += 1;
                self.draw_obj_8x8_tile(&obj, y, tile_id, y >> 3, size);
            }
        }
    }

    pub fn draw_scanline(&mut self) {
        let y = self.pos.y + 1;
        let mut n = usize::from(self.pos.y) * 256;
        for bg in &mut self.bgs {
            bg.cached_tile = None;
        }
        for bg in &mut self.bgs {
            if bg.mosaic && bg.mosaic_start.is_none() {
                bg.mosaic_start = Some(y);
            }
        }
        if self.force_blank {
            self.frame_buffer.mut_pixels()[n..n + 256].fill([0; 4])
        } else {
            self.refill_obj_cache(y);
            self.mode7_settings.tmpy = (y & 0xff) as u8;
            if self.mode7_settings.y_mirror {
                self.mode7_settings.tmpy ^= 0xff;
            }
            self.mode7_settings.update_tmp3::<0>();
            self.mode7_settings.update_tmp3::<1>();
            for x in 0u8..=255 {
                self.frame_buffer.mut_pixels()[n] = self.draw_pixel(x, y);
                n += 1;
            }
        }
    }

    pub fn is_in_window(&self, x: u8, window: &Window) -> bool {
        let window_n = |n: usize| {
            (self.window_positions[n][0]..=self.window_positions[n][1]).contains(&x)
                ^ window.window_inversion[n]
        };
        match window.windows {
            [false, false] => false,
            [true, false] => window_n(0),
            [false, true] => window_n(1),
            [true, true] => match window.mask_logic {
                MaskLogic::Or => window_n(0) || window_n(1),
                MaskLogic::And => window_n(0) && window_n(1),
                MaskLogic::Xor => window_n(0) ^ window_n(1),
                MaskLogic::XNor => window_n(0) == window_n(1),
            },
        }
    }

    pub fn layers_mut(&mut self) -> impl Iterator<Item = &mut Layer> {
        self.bgs
            .iter_mut()
            .map(|bg| &mut bg.layer)
            .chain(core::iter::once(&mut self.obj_layer))
    }

    pub fn get_pos(&self) -> &RayPos {
        &self.pos
    }

    pub fn mut_pos(&mut self) -> &mut RayPos {
        &mut self.pos
    }

    pub fn latch(&mut self) {
        self.latched.pos = self.pos;
        self.latched.latched = true
    }

    pub fn is_interlaced(&self) -> bool {
        self.interlace_active
    }

    pub fn is_overscan(&self) -> bool {
        self.overscan
    }

    pub fn is_field(&self) -> bool {
        self.field
    }

    pub fn get_scanline_cycles(&self) -> u16 {
        if !self.is_pal && !self.is_interlaced() && self.field && self.pos.y == 240 {
            1360
        } else if self.is_pal && self.is_interlaced() && self.field && self.pos.y == 311 {
            1368
        } else {
            1364
        }
    }

    pub fn get_scanline_count(&self) -> u16 {
        if self.is_pal {
            312
        } else {
            262
        }
    }

    pub fn is_in_hblank_reg4212(&self) -> bool {
        !(3..=1095).contains(&self.pos.x)
    }

    pub fn is_in_vblank(&self) -> bool {
        self.pos.y >= self.vend()
    }

    pub fn is_cpu_active(&self) -> bool {
        !(536..536 + 40).contains(&self.pos.x)
    }

    pub fn end_vblank(&mut self) {
        self.field ^= true;
        self.bgs.iter_mut().for_each(|bg| bg.mosaic_start = None);
        if !self.force_blank {
            self.overflow_flags = 0;
        }
    }

    pub fn vend(&self) -> u16 {
        (if self.overscan {
            MAX_SCREEN_HEIGHT_OVERSCAN
        } else {
            MAX_SCREEN_HEIGHT
        } + 1) as _
    }

    pub fn vblank(&mut self) {
        if !self.force_blank {
            self.oam.oam_reset();
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, InSaveState)]
pub struct RemapMode {
    mask: u16,
    shift: u8,
}

impl RemapMode {
    pub fn new(val: u8) -> Self {
        core::num::NonZeroU8::new(val & 3)
            .map(|v| Self {
                mask: (1 << (v.get() + 7)) - 1,
                shift: v.get() | 4,
            })
            .unwrap_or_default()
    }

    pub const fn remap(&self, addr: u16) -> u16 {
        let addr_part = addr & !self.mask;
        let rest_part = addr & self.mask;
        (((rest_part >> self.shift) | (rest_part << 3)) & self.mask) | addr_part
    }
}
