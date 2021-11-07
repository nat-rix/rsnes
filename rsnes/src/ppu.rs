use crate::oam::{CgRam, Oam};
use core::mem::replace;

pub const VRAM_SIZE: usize = 0x8000;
pub const SCREEN_WIDTH: u32 = 256;
pub const MAX_SCREEN_HEIGHT: u32 = 239;
pub const CHIP_5C78_VERSION: u8 = 3;

#[derive(Debug, Clone, Copy)]
pub enum BgModeNum {
    Mode0,
    Mode1,
    Mode2,
    Mode3,
    Mode4,
    Mode5,
    Mode6,
    Mode7,
}

#[derive(Debug, Clone, Copy)]
pub struct BgMode {
    num: BgModeNum,
    // only relevant to mode 1
    bg3_priority: bool,
    // only relevant to mode 7
    extbg: bool,
}

#[derive(Debug, Clone, Copy)]
enum DrawLayer {
    Bg(u8, u8, bool),
    Sprite(u8),
}

impl BgMode {
    pub fn new() -> Self {
        Self {
            num: BgModeNum::Mode0,
            bg3_priority: false,
            extbg: false,
        }
    }

    pub fn set_bits(&mut self, bits: u8) {
        self.num = match bits & 7 {
            0 => BgModeNum::Mode0,
            1 => BgModeNum::Mode1,
            2 => BgModeNum::Mode2,
            3 => BgModeNum::Mode3,
            4 => BgModeNum::Mode4,
            5 => BgModeNum::Mode5,
            6 => BgModeNum::Mode6,
            7 => BgModeNum::Mode7,
            _ => unreachable!(),
        };
        self.bg3_priority = bits & 8 > 0;
    }

    fn get_layers(&self) -> &'static [DrawLayer] {
        use BgModeNum::*;
        const fn s(prio: u8) -> DrawLayer {
            DrawLayer::Sprite(prio)
        }
        const fn b(bg: u8, depth: u8, prio: u8) -> DrawLayer {
            DrawLayer::Bg(bg - 1, depth, prio == 1)
        }
        static MODE0: [DrawLayer; 12] = [
            s(3),
            b(1, 1, 1),
            b(2, 1, 1),
            s(2),
            b(1, 1, 0),
            b(2, 1, 0),
            s(1),
            b(3, 1, 1),
            b(4, 1, 1),
            s(0),
            b(3, 1, 0),
            b(4, 1, 0),
        ];
        static MODE1: [DrawLayer; 10] = [
            s(3),
            b(1, 2, 1),
            b(2, 2, 1),
            s(2),
            b(1, 2, 0),
            b(2, 2, 0),
            s(1),
            b(3, 1, 1),
            s(0),
            b(3, 1, 0),
        ];
        static MODE1_BG3: [DrawLayer; 10] = [
            b(3, 1, 1),
            s(3),
            b(1, 2, 1),
            b(2, 2, 1),
            s(2),
            b(1, 2, 0),
            b(2, 2, 0),
            s(1),
            s(0),
            b(3, 1, 0),
        ];
        static MODE2: [DrawLayer; 8] = [
            s(3),
            b(1, 2, 1),
            s(2),
            b(2, 2, 1),
            s(1),
            b(1, 2, 0),
            s(0),
            b(2, 2, 0),
        ];
        static MODE3: [DrawLayer; 8] = [
            s(3),
            b(1, 3, 1),
            s(2),
            b(2, 2, 1),
            s(1),
            b(1, 3, 0),
            s(0),
            b(2, 2, 0),
        ];
        static MODE4: [DrawLayer; 8] = [
            s(3),
            b(1, 3, 1),
            s(2),
            b(2, 1, 1),
            s(1),
            b(1, 3, 0),
            s(0),
            b(2, 1, 0),
        ];
        static MODE5: [DrawLayer; 8] = [
            s(3),
            b(1, 2, 1),
            s(2),
            b(2, 1, 1),
            s(1),
            b(1, 2, 0),
            s(0),
            b(2, 1, 0),
        ];
        static MODE6: [DrawLayer; 6] = [s(3), b(1, 2, 1), s(2), s(1), b(1, 2, 0), s(0)];
        static MODE7: [DrawLayer; 5] = [s(3), s(2), s(1), b(1, 3, 0), s(0)];
        static MODE7_EXTBG: [DrawLayer; 7] = [
            s(3),
            s(2),
            b(2, 0xff, 1),
            s(1),
            b(1, 3, 0),
            s(0),
            b(2, 0xff, 0),
        ];
        match self.num {
            Mode0 => &MODE0,
            Mode1 => {
                if self.bg3_priority {
                    &MODE1_BG3
                } else {
                    &MODE1
                }
            }
            Mode2 => &MODE2,
            Mode3 => &MODE3,
            Mode4 => &MODE4,
            Mode5 => &MODE5,
            Mode6 => &MODE6,
            Mode7 => {
                if self.extbg {
                    &MODE7_EXTBG
                } else {
                    &MODE7
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Background {
    tilemap_addr: u8,
    base_addr: u8,
    x_mirror: bool,
    y_mirror: bool,
    // otherwise it is 8x8
    is_16x16_tiles: bool,
    scroll_prev: u8,
    scroll_prev_h: u8,
    scroll: [u16; 2],
    layer: Layer,
}

impl Background {
    pub const fn new() -> Self {
        Self {
            tilemap_addr: 0,
            base_addr: 0,
            x_mirror: false,
            y_mirror: false,
            is_16x16_tiles: false,
            scroll_prev: 0,
            scroll_prev_h: 0,
            scroll: [0; 2],
            layer: Layer::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Layer {
    mask_logic: MaskLogic,
    windows: [bool; 2],
    window_inversion: [bool; 2],
    color_math: bool,
    // not on color_layer
    main_screen: bool,
    // not on color_layer
    sub_screen: bool,
    // not on color_layer
    main_screen_masked: bool,
    // not on color_layer
    sub_screen_masked: bool,
}

impl Layer {
    pub const fn new() -> Self {
        Self {
            mask_logic: MaskLogic::Or,
            windows: [false; 2],
            window_inversion: [false; 2],
            color_math: false,
            main_screen: false,
            sub_screen: false,
            main_screen_masked: false,
            sub_screen_masked: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskLogic {
    Or,
    And,
    Xor,
    XNor,
}

impl MaskLogic {
    pub fn from_byte(val: u8) -> Self {
        match val & 3 {
            0 => Self::Or,
            1 => Self::And,
            2 => Self::Xor,
            3 => Self::XNor,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Mode7Settings {
    x_mirror: bool,
    y_mirror: bool,
    fill_zeros: Option<bool>,
    prev: u8,
    offset: [i16; 2],
    matrix: [u16; 4],
    center: [i16; 2],
}

impl Mode7Settings {
    pub fn write_offset(&mut self, is_vertical: bool, val: u8) {
        let i = is_vertical as usize;
        let mut val = u16::from(replace(&mut self.prev, val)) | (u16::from(val) << 8);
        if val & 0x1000 > 0 {
            val |= 0xe000
        }
        self.offset[i] = val as i16
    }

    pub fn set_matrix(&mut self, entry: u8, val: u8) {
        self.matrix[usize::from(entry)] =
            (u16::from(val) << 8) | u16::from(replace(&mut self.prev, val))
    }

    pub fn set_center(&mut self, entry: u8, val: u8) {
        let mut val = (u16::from(val) << 8) | u16::from(replace(&mut self.prev, val));
        if val & 0x1000 > 0 {
            val |= 0xe000
        }
        self.center[usize::from(entry)] = val as i16
    }
}

#[derive(Debug, Clone)]
pub struct Ppu<FB: crate::backend::FrameBuffer> {
    pub(crate) oam: Oam,
    pub frame_buffer: FB,
    cgram: CgRam,
    vram: [u16; VRAM_SIZE],
    vram_addr_unmapped: u16,
    /// A value between 0 and 15 with 15 being maximum brightness
    brightness: u8,
    obj_size: ObjectSize,
    remap_mode: RemapMode,
    vram_increment_amount: u8,
    increment_first: bool,
    pub(crate) overscan: bool,
    mosaic_bgs: u8,
    mosaic_size: u8,
    bgs: [Background; 4],
    obj_layer: Layer,
    color_layer: Layer,
    mode7_settings: Mode7Settings,
    direct_color_mode: bool,
    add_subscreen: bool,
    color_behaviour: u8,
    subtract_color: bool,
    half_color: bool,
    fixed_color: Color,
    bg_mode: BgMode,
    window_positions: [[u8; 2]; 2],
    force_blank: bool,
    open_bus1: u8,
    open_bus2: u8,
}

impl<FB: crate::backend::FrameBuffer> Ppu<FB> {
    pub fn new(frame_buffer: FB) -> Self {
        Self {
            oam: Oam::new(),
            frame_buffer,
            cgram: CgRam::new(),
            vram: [0; VRAM_SIZE],
            vram_addr_unmapped: 0,
            brightness: 0x0f,
            obj_size: ObjectSize::O8S16,
            remap_mode: RemapMode::NoRemap,
            vram_increment_amount: 1,
            increment_first: true,
            overscan: false,
            mosaic_bgs: 0,
            mosaic_size: 0,
            bgs: [Background::new(); 4],
            obj_layer: Layer::new(),
            color_layer: Layer::new(),
            mode7_settings: Mode7Settings {
                x_mirror: false,
                y_mirror: false,
                fill_zeros: None,
                prev: 0,
                offset: [0; 2],
                matrix: [0; 4],
                center: [0; 2],
            },
            direct_color_mode: false,
            add_subscreen: false,
            color_behaviour: 0,
            subtract_color: false,
            half_color: false,
            fixed_color: Color::BLACK,
            bg_mode: BgMode::new(),
            window_positions: [[0; 2]; 2],
            force_blank: true,
            open_bus1: 0,
            open_bus2: 0,
        }
    }

    /// Read from a PPU register (memory map 0x2134..=0x213f)
    pub fn read_register(&mut self, id: u8) -> Option<u8> {
        match id {
            0x3f => {
                // STAT78
                // TODO: support interlace
                // TODO: implement counter latching
                // TODO: implement PAL mode
                let val = (self.open_bus2 & 0x20) | CHIP_5C78_VERSION;
                self.open_bus2 = val;
                Some(val)
            }
            0x34..=0x36 => {
                // MPYx
                self.open_bus1 = (((u32::from(self.mode7_settings.matrix[0])
                    * (u32::from(self.mode7_settings.matrix[1]) >> 8))
                    >> ((id & 3) << 3))
                    & 0xff) as u8;
                Some(self.open_bus1)
            }
            _ => todo!("read from unknown PPU register 0x21{:02x}", id),
        }
    }

    /// Write to a PPU register (memory map 0x2100..=0x2133)
    pub fn write_register(&mut self, id: u8, val: u8) {
        match id {
            0x00 => {
                // INIDISP
                self.force_blank = val & 0x80 > 0;
                self.brightness = val & 0b1111;
            }
            0x01 => {
                // OBSEL
                // TODO: name select bits and name base select bits
                self.obj_size = ObjectSize::from_upper_bits(val);
            }
            0x02 => {
                // OAMADDL
                self.oam.set_addr_low(val)
            }
            0x03 => {
                // OAMADDH
                self.oam.set_addr_high(val)
            }
            0x04 => {
                // OAMDATA
                self.oam.write(val)
            }
            0x05 => {
                // BGMODE
                self.bg_mode.set_bits(val);
                for (i, bg) in self.bgs.iter_mut().enumerate() {
                    bg.is_16x16_tiles = val & (1 << (i | 4)) > 0;
                }
            }
            0x06 => {
                // MOSAIC
                self.mosaic_size = val >> 4;
                self.mosaic_bgs = val & 0xf;
            }
            0x07..=0x0a => {
                // BGnSC
                let bg = &mut self.bgs[usize::from((id + 1) & 3)];
                bg.tilemap_addr = val & 0xfc;
                bg.y_mirror = val & 2 > 0;
                bg.x_mirror = val & 1 > 0;
            }
            0x0b..=0x0c => {
                // BGnmNBA
                let val = val & 0x77;
                let id = usize::from(!id & 2);
                self.bgs[id].base_addr = val & 0xf;
                self.bgs[id | 1].base_addr = val >> 4;
            }
            0x0d..=0x14 => {
                // M7xOFS and BGnxOFS
                if (0x0d..=0x0e).contains(&id) {
                    self.mode7_settings.write_offset(id & 1 == 0, val)
                }
                let bg = &mut self.bgs[usize::from(((id - 5) >> 1) & 3)];
                let old = replace(&mut bg.scroll_prev, val);
                bg.scroll[usize::from(!id & 1)] = (u16::from(val) << 8)
                    | u16::from(if id & 1 > 0 {
                        (old & 0xf8) | replace(&mut bg.scroll_prev_h, val) & 7
                    } else {
                        old
                    });
            }
            0x15 => {
                // VMAIN - Video Port Control
                self.increment_first = val & 0x80 == 0;
                self.vram_increment_amount = match val & 0b11 {
                    0 => 1,
                    1 => 32,
                    _ => 128,
                };
                self.remap_mode = RemapMode::from_bits(val >> 2);
            }
            0x16 => {
                // VMADDL
                self.vram_addr_unmapped = (self.vram_addr_unmapped & 0xff00) | u16::from(val);
            }
            0x17 => {
                // VMADDH
                self.vram_addr_unmapped = (self.vram_addr_unmapped & 0xff) | (u16::from(val) << 8);
            }
            0x18 => {
                // VMDATAL
                let word = self.get_vram_word_mut(self.vram_addr_unmapped);
                *word = (*word & 0xff00) | u16::from(val);
                if self.increment_first {
                    self.vram_addr_unmapped = self
                        .vram_addr_unmapped
                        .wrapping_add(self.vram_increment_amount.into());
                }
            }
            0x19 => {
                // VMDATAH
                let word = self.get_vram_word_mut(self.vram_addr_unmapped);
                *word = (*word & 0xff) | (u16::from(val) << 8);
                if !self.increment_first {
                    self.vram_addr_unmapped = self
                        .vram_addr_unmapped
                        .wrapping_add(self.vram_increment_amount.into());
                }
            }
            0x1a => {
                // M7SEL
                self.mode7_settings.x_mirror = val & 1 > 0;
                self.mode7_settings.y_mirror = val & 2 > 0;
                self.mode7_settings.fill_zeros = Some(val & 0x40 > 0).filter(|_| val & 0x80 > 0);
            }
            0x1b..=0x1e => {
                // M7x
                self.mode7_settings.set_matrix((id + 1) & 3, val)
            }
            0x1f | 0x20 => {
                // M7X/M7Y
                self.mode7_settings.set_center(!id & 1, val)
            }
            0x21 => {
                // CGADD
                self.cgram.set_addr(val)
            }
            0x22 => {
                // CGADD
                self.cgram.write(val)
            }
            0x23..=0x24 => {
                // WnmSEL
                let mut val = val;
                for i in 0..2 {
                    let bg = &mut self.bgs[usize::from(i + (!id & 2))];
                    bg.layer.window_inversion[0] = val & 1 > 0;
                    bg.layer.windows[0] = val & 2 > 0;
                    bg.layer.window_inversion[1] = val & 4 > 0;
                    bg.layer.windows[1] = val & 8 > 0;
                    val >>= 4;
                }
            }
            0x25 => {
                // WOBJSEL
                let mut val = val;
                for layer in [&mut self.obj_layer, &mut self.color_layer] {
                    layer.window_inversion[0] = val & 1 > 0;
                    layer.windows[0] = val & 2 > 0;
                    layer.window_inversion[1] = val & 4 > 0;
                    layer.windows[1] = val & 8 > 0;
                    val >>= 4;
                }
            }
            0x26..=0x29 => {
                // WH0-3
                self.window_positions[usize::from((!id & 2) >> 1)][usize::from(id & 1)] = val
            }
            0x2a => {
                // WBGLOG
                for i in 0..4 {
                    self.bgs[i].layer.mask_logic = MaskLogic::from_byte(val >> (i << 1));
                }
            }
            0x2b => {
                // WOBJLOG
                self.obj_layer.mask_logic = MaskLogic::from_byte(val);
                self.color_layer.mask_logic = MaskLogic::from_byte(val >> 2);
            }
            0x2c..=0x2f => {
                // TM/TS/TMW/TSW
                let f: fn(&mut Layer, val: bool) = match id {
                    0x2c => |layer, val| layer.main_screen = val,
                    0x2d => |layer, val| layer.sub_screen = val,
                    0x2e => |layer, val| layer.main_screen_masked = val,
                    0x2f => |layer, val| layer.sub_screen_masked = val,
                    _ => unreachable!(),
                };
                for (i, bg) in self.bgs.iter_mut().enumerate() {
                    f(&mut bg.layer, val & (1 << i) > 0)
                }
                f(&mut self.obj_layer, val & 0x10 > 0)
            }
            0x30 => {
                // CGWSEL
                self.direct_color_mode = val & 1 > 0;
                self.add_subscreen = val & 2 > 0;
                self.color_behaviour = val >> 4;
            }
            0x31 => {
                // CGADSUB
                self.bgs[0].layer.color_math = val & 1 > 0;
                self.bgs[1].layer.color_math = val & 2 > 0;
                self.bgs[2].layer.color_math = val & 4 > 0;
                self.bgs[3].layer.color_math = val & 8 > 0;
                self.obj_layer.color_math = val & 0x10 > 0;
                self.color_layer.color_math = val & 0x20 > 0;
                self.half_color = val & 0x40 > 0;
                self.subtract_color = val & 0x80 > 0;
            }
            0x32 => {
                // COLDATA
                if val & 0x20 > 0 {
                    self.fixed_color.r = val
                }
                if val & 0x40 > 0 {
                    self.fixed_color.g = val
                }
                if val & 0x80 > 0 {
                    self.fixed_color.b = val
                }
            }
            0x33 => {
                // SETINI
                if val & 1 > 0 {
                    todo!("screen interlace mode")
                }
                if val & 2 > 0 {
                    todo!("object interlace mode")
                }
                self.overscan = val & 4 > 0;
                if val & 8 > 0 {
                    todo!("pseudo-hires mode")
                }
                self.bg_mode.extbg = val & 0x40 > 0;
                if val & 0x80 > 0 {
                    todo!("enable super imposing")
                }
            }
            _ => todo!("write to unknown PPU register 0x21{:02x}", id),
        }
    }

    fn get_vram_word_mut(&mut self, index: u16) -> &mut u16 {
        self.vram
            .get_mut(usize::from(self.remap_mode.remap(index) & 0x7fff))
            .unwrap()
    }

    fn get_layer_by_info(&self, layer_info: &DrawLayer) -> &Layer {
        match layer_info {
            DrawLayer::Bg(n, _, _) => &self.bgs[usize::from(*n)].layer,
            DrawLayer::Sprite(_) => &self.obj_layer,
        }
    }

    fn is_in_window(&self, x: u16, layer: &Layer) -> bool {
        let window_n = |n: usize| -> bool {
            (self.window_positions[n][0]..=self.window_positions[n][1])
                .contains(&((x & 0xff) as u8))
                ^ layer.window_inversion[n]
        };
        match layer.windows {
            [false, false] => false,
            [false, true] => window_n(1),
            [true, false] => window_n(0),
            [true, true] => match layer.mask_logic {
                MaskLogic::Or => window_n(0) || window_n(1),
                MaskLogic::And => window_n(0) && window_n(1),
                MaskLogic::Xor => window_n(0) ^ window_n(1),
                MaskLogic::XNor => window_n(0) == window_n(1),
            },
        }
    }

    fn get_bg_color(
        &self,
        bg_nr: u8,
        bit_depth: u8,
        priority: bool,
        [x, y]: [u16; 2],
    ) -> Option<Color> {
        let bg = &self.bgs[usize::from(bg_nr)];
        let [x, y] = [(x + bg.scroll[0]) & 0x3ff, (y + bg.scroll[1]) & 0x3ff];
        let is_y16 = bg.is_16x16_tiles;
        let is_x16 = is_y16 || matches!(self.bg_mode.num, BgModeNum::Mode5 | BgModeNum::Mode6);
        let xbits = if is_x16 { 4 } else { 3 };
        let ybits = if is_y16 { 4 } else { 3 };
        let mut tilemap_addr = (u16::from(bg.tilemap_addr) << 8)
            .wrapping_add((((y >> ybits) & 0x1f) << 5) | ((x >> xbits) & 0x1f));
        if bg.x_mirror && x & (0x20 << xbits) > 0 {
            tilemap_addr = tilemap_addr.wrapping_add(0x400)
        }
        if bg.y_mirror && y & (0x20 << ybits) > 0 {
            tilemap_addr = tilemap_addr.wrapping_add(0x400);
            if bg.x_mirror {
                tilemap_addr = tilemap_addr.wrapping_add(0x400);
            }
        }
        let tile_info = self.vram[usize::from(tilemap_addr & 0x7fff)];
        if priority ^ ((tile_info & 0x2000) > 0) {
            return None;
        }
        let palette = ((tile_info >> 10) & 7) as u8;
        let palette = if let BgModeNum::Mode0 = self.bg_mode.num {
            palette | (bg_nr << 3)
        } else {
            palette
        };
        let tx = if tile_info & 0x4000 > 0 { x } else { !x } & 7;
        let ty = if tile_info & 0x8000 > 0 { !y } else { y } & 7;
        let mut tile_nr = tile_info & 0x3ff;
        if is_x16 && ((x & 8 > 0) ^ (tile_info & 0x4000 > 0)) {
            tile_nr += 1;
        }
        if is_y16 && ((y & 8 > 0) ^ (tile_info & 0x8000 > 0)) {
            tile_nr += 16;
        }
        let plane = self.vram[usize::from(
            (u16::from(bg.base_addr) << 12)
                .wrapping_add((tile_nr & 0x3ff) << (2 + bit_depth))
                .wrapping_add(ty)
                & 0x7fff,
        )];
        let mut pixel = ((plane >> tx) & 1) | ((plane >> (tx + 7)) & 2);
        let mut palette_dimensions = 2;
        if bit_depth > 1 {
            palette_dimensions = 4;
            let plane = self.vram[usize::from(
                (u16::from(bg.base_addr) << 12)
                    .wrapping_add((tile_nr & 0x3ff) << (2 + bit_depth))
                    .wrapping_add(ty)
                    .wrapping_add(8)
                    & 0x7fff,
            )];
            pixel |= (((plane >> tx) & 1) | ((plane >> (tx + 7)) & 2)) << 2;
        }
        if bit_depth > 2 {
            palette_dimensions = 8;
            let plane = self.vram[usize::from(
                (u16::from(bg.base_addr) << 12)
                    .wrapping_add((tile_nr & 0x3ff) << (2 + bit_depth))
                    .wrapping_add(ty)
                    .wrapping_add(16)
                    & 0x7fff,
            )];
            pixel |= (((plane >> tx) & 1) | ((plane >> (tx + 7)) & 2)) << 4;
            let plane = self.vram[usize::from(
                (u16::from(bg.base_addr) << 12)
                    .wrapping_add((tile_nr & 0x3ff) << (2 + bit_depth))
                    .wrapping_add(ty)
                    .wrapping_add(32)
                    & 0x7fff,
            )];
            pixel |= (((plane >> tx) & 1) | ((plane >> (tx + 7)) & 2)) << 6;
        }
        if pixel == 0 {
            None
        } else {
            let pixel = u32::from(pixel).wrapping_add(u32::from(palette << palette_dimensions));
            self.pixel_to_color(pixel, bit_depth)
        }
    }

    fn pixel_to_color(&self, pixel: u32, bit_depth: u8) -> Option<Color> {
        let mut color = if self.direct_color_mode && bit_depth == 3 {
            Color::new(
                (((pixel & 0x7) << 2) | ((pixel & 0x100) >> 7)) as u8,
                (((pixel & 0x38) >> 1) | ((pixel & 0x200) >> 8)) as u8,
                (((pixel & 0xc0) >> 3) | ((pixel & 0x400) >> 8)) as u8,
            )
        } else {
            let color = self.cgram.read16((pixel & 0xff) as u8);
            Color::new(
                (color & 0x1f) as u8,
                ((color >> 5) & 0x1f) as u8,
                ((color >> 10) & 0x1f) as u8,
            )
        };
        color.r <<= 3;
        color.g <<= 3;
        color.b <<= 3;
        Some(color)
    }

    fn get_bg7_color(
        &self,
        bg_nr: u8,
        bit_depth: u8,
        priority: bool,
        x: u16,
        m7_precalc: &[i32; 2],
    ) -> Option<Color> {
        let x = (x & 0xff) as u8;
        let x = if self.mode7_settings.x_mirror { !x } else { x };

        let pixel = [
            (m7_precalc[0] + self.mode7_settings.matrix[0] as i16 as i32 * x as i32) >> 8,
            (m7_precalc[1] + self.mode7_settings.matrix[2] as i16 as i32 * x as i32) >> 8,
        ];
        let palette_addr = ((((pixel[1] as u32) & 7) as u8) << 3) | (pixel[0] as u32 & 0x7) as u8;
        let tile_addr =
            ((pixel[0] >> 3) as u32 & 0x7f) as u16 | (((pixel[1] >> 3) as u32 & 0x7f) << 7) as u16;
        let out_of_bounds = !(0..1024).contains(&pixel[0]) || !(0..1024).contains(&pixel[1]);
        let tile = if let Some(true) = self.mode7_settings.fill_zeros.filter(|_| out_of_bounds) {
            0
        } else {
            (self.vram[usize::from(tile_addr)] & 0xff) as u8
        };
        let pixel = if let Some(false) = self.mode7_settings.fill_zeros.filter(|_| out_of_bounds) {
            0
        } else {
            (self.vram[usize::from((u16::from(tile) << 6) | u16::from(palette_addr))] >> 8) as u8
        };
        self.pixel_to_color(
            (if bg_nr == 1 {
                if (pixel & 0x80 == 0) == priority {
                    return None;
                }
                pixel & 0x7f
            } else {
                pixel
            })
            .into(),
            bit_depth,
        )
    }

    fn get_sprite_color(&self, priority: u8, [x, y]: [u16; 2]) -> Option<Color> {
        None
    }

    fn fetch_pixel_layer(
        &mut self,
        [x, y]: [u16; 2],
        m7_precalc: &Option<[i32; 2]>,
    ) -> (u8, Color) {
        let brightness = self.brightness as f32 / 15.0;
        let mut color = None;
        for (i, layer_info) in self.bg_mode.get_layers().iter().enumerate() {
            let layer = self.get_layer_by_info(layer_info);
            color = if layer.main_screen
                && !(layer.main_screen_masked && self.is_in_window(x, layer))
            {
                match layer_info {
                    DrawLayer::Bg(bg_nr, bit_depth, priority) => {
                        if let Some(m7_precalc) = m7_precalc {
                            self.get_bg7_color(*bg_nr, *bit_depth, *priority, x, m7_precalc)
                        } else {
                            self.get_bg_color(*bg_nr, *bit_depth, *priority, [x, y])
                        }
                    }
                    DrawLayer::Sprite(priority) => self.get_sprite_color(*priority, [x, y]),
                }
            } else {
                None
            };
            if color.is_some() {
                break;
            }
        }
        (
            0,
            color.unwrap_or_else(|| {
                Color::new(
                    (self.fixed_color.r as f32 / 32.0 * 255.0 * brightness) as u8,
                    (self.fixed_color.g as f32 / 32.0 * 255.0 * brightness) as u8,
                    (self.fixed_color.b as f32 / 32.0 * 255.0 * brightness) as u8,
                )
            }),
        )
    }

    fn fetch_pixel(&mut self, [x, y]: [u16; 2], m7_precalc: &Option<[i32; 2]>) -> Color {
        if self.force_blank {
            Color::BLACK
        } else {
            let (_layer, color) = self.fetch_pixel_layer([x, y], m7_precalc);
            color
        }
    }

    pub fn draw_line(&mut self, y: u16) {
        if y == 0 {
            return;
        }
        let m7_precalc = if let BgModeNum::Mode7 = self.bg_mode.num {
            let y = (y & 0xff) as u8;
            let y = if self.mode7_settings.y_mirror { !y } else { y };

            let dif = [
                self.mode7_settings.offset[0].wrapping_sub(self.mode7_settings.center[0]),
                self.mode7_settings.offset[1].wrapping_sub(self.mode7_settings.center[1]),
            ];
            let clip = |x: u16| {
                (if x & 0x2000 > 0 {
                    x | 0xfc00
                } else {
                    x & 0x3ff
                }) as i16
            };
            let dif = [clip(dif[0] as u16), clip(dif[1] as u16)];
            let origin = |a, b, c| {
                ((i32::from(a) * i32::from(dif[0])) & -64i32)
                    + ((i32::from(b) * i32::from(dif[1])) & -64i32)
                    + ((i32::from(b) * i32::from(y)) & -64i32)
                    + (i32::from(c) << 8)
            };
            Some([
                origin(
                    self.mode7_settings.matrix[0] as i16,
                    self.mode7_settings.matrix[1] as i16,
                    self.mode7_settings.center[0],
                ),
                origin(
                    self.mode7_settings.matrix[2] as i16,
                    self.mode7_settings.matrix[3] as i16,
                    self.mode7_settings.center[1],
                ),
            ])
        } else {
            None
        };
        let offset = u32::from(y - 1) * SCREEN_WIDTH;
        for x in 0..SCREEN_WIDTH as u16 {
            let Color { r, g, b } = self.fetch_pixel([x, y], &m7_precalc);
            self.frame_buffer.mut_pixels()[(offset + u32::from(x)) as usize] = [r, g, b, 0];
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectSize {
    /// Object 8x8 - Sprite 16x16
    O8S16,
    /// Object 8x8 - Sprite 32x32
    O8S32,
    /// Object 8x8 - Sprite 64x64
    O8S64,
    /// Object 16x16 - Sprite 32x32
    O16S32,
    /// Object 16x16 - Sprite 64x64
    O16S64,
    /// Object 32x32 - Sprite 64x64
    O32S64,
    /// Object 16x32 - Sprite 32x64
    O16x32S32x64,
    /// Object 16x32 - Sprite 32x32
    O16x32S32,
}

impl ObjectSize {
    pub fn from_upper_bits(bits: u8) -> Self {
        match bits >> 5 {
            0b000 => Self::O8S16,
            0b001 => Self::O8S32,
            0b010 => Self::O8S64,
            0b011 => Self::O16S32,
            0b100 => Self::O16S64,
            0b101 => Self::O32S64,
            0b110 => Self::O16x32S32x64,
            0b111 => Self::O16x32S32,
            _ => unreachable!(),
        }
    }

    pub const fn get_obj_width(&self) -> u8 {
        match self {
            Self::O8S16 | Self::O8S32 | Self::O8S64 => 8,
            Self::O16S32 | Self::O16S64 | Self::O16x32S32 | Self::O16x32S32x64 => 16,
            Self::O32S64 => 32,
        }
    }

    pub const fn get_obj_height(&self) -> u8 {
        match self {
            Self::O8S16 | Self::O8S32 | Self::O8S64 => 8,
            Self::O16S32 | Self::O16S64 => 16,
            Self::O32S64 | Self::O16x32S32 | Self::O16x32S32x64 => 32,
        }
    }

    pub const fn get_sprite_width(&self) -> u8 {
        match self {
            Self::O8S16 => 16,
            Self::O8S32 | Self::O16S32 | Self::O16x32S32 | Self::O16x32S32x64 => 32,
            Self::O8S64 | Self::O16S64 | Self::O32S64 => 64,
        }
    }

    pub const fn get_sprite_height(&self) -> u8 {
        match self {
            Self::O8S16 => 16,
            Self::O8S32 | Self::O16S32 | Self::O16x32S32 => 32,
            Self::O8S64 | Self::O16S64 | Self::O32S64 | Self::O16x32S32x64 => 64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemapMode {
    NoRemap,
    First,
    Second,
    Third,
}

impl RemapMode {
    pub fn from_bits(bits: u8) -> Self {
        match bits & 0b11 {
            0b00 => Self::NoRemap,
            0b01 => Self::First,
            0b10 => Self::Second,
            0b11 => Self::Third,
            _ => unreachable!(),
        }
    }

    pub const fn remap(&self, addr: u16) -> u16 {
        match self {
            Self::NoRemap => addr,
            Self::First => (addr & 0xff00) | ((addr & 0x1f) << 3) | ((addr >> 5) & 0b111),
            Self::Second => (addr & 0xfe00) | ((addr & 0x3f) << 3) | ((addr >> 6) & 0b111),
            Self::Third => (addr & 0xfc00) | ((addr & 0x7f) << 3) | ((addr >> 7) & 0b111),
        }
    }
}
