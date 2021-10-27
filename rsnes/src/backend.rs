mod picture {
    pub trait PictureBackend {}
    pub struct Dummy;

    impl PictureBackend for Dummy {}
}

mod audio {
    use crate::spc700::StereoSample;

    pub trait AudioBackend {
        fn push_sample(&mut self, sample: StereoSample<i16>);
    }
    pub struct Dummy;

    impl AudioBackend for Dummy {
        fn push_sample(&mut self, _sample: StereoSample<i16>) {}
    }
}

pub use audio::{AudioBackend, Dummy as AudioDummy};
pub use picture::{Dummy as PictureDummy, PictureBackend};

pub trait Backend {
    type Audio: AudioBackend;
    type Picture: PictureBackend;
}

pub struct Dummy;

impl Backend for Dummy {
    type Audio = AudioDummy;
    type Picture = PictureDummy;
}
