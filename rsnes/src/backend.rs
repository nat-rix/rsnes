mod audio {
    use crate::spc700::StereoSample;

    pub trait AudioBackend {
        fn push_sample(&mut self, sample: StereoSample);
    }
    pub struct Dummy;

    impl AudioBackend for Dummy {
        fn push_sample(&mut self, _sample: StereoSample) {}
    }
}

pub use audio::{AudioBackend, Dummy as AudioDummy};

pub trait FrameBuffer {
    fn pixels(&self) -> &[[u8; 4]];
    fn mut_pixels(&mut self) -> &mut [[u8; 4]];
    fn request_redraw(&mut self);
}

pub const FRAME_BUFFER_SIZE: usize = (ppu::MAX_SCREEN_HEIGHT * ppu::SCREEN_WIDTH) as usize;
use crate::ppu;
#[derive(Debug, Clone)]
pub struct ArrayFrameBuffer(pub [[u8; 4]; FRAME_BUFFER_SIZE], pub bool);

impl FrameBuffer for ArrayFrameBuffer {
    fn pixels(&self) -> &[[u8; 4]] {
        &self.0
    }
    fn mut_pixels(&mut self) -> &mut [[u8; 4]] {
        &mut self.0
    }
    fn request_redraw(&mut self) {
        self.1 = true
    }
}

impl ArrayFrameBuffer {
    pub fn get_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.0.as_ptr() as _, self.0.len() << 2) }
    }
}
