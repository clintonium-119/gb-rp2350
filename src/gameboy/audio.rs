

pub struct NullAudioPlayer;

impl gb_core::hardware::sound::AudioPlayer for NullAudioPlayer {
    fn play(&mut self, _buf_left: &[f32], _buf_right: &[f32]) {
        // Do nothing
    }

    fn samples_rate(&self) -> u32 {
        5512
        //1098
    }

    fn underflowed(&self) -> bool {
        false
    }
}