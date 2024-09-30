pub struct NullAudioPlayer;

impl gb_core::hardware::sound::AudioPlayer for NullAudioPlayer {
    fn play(&mut self, _output_buffer: &[u16]) {
        // Do nothing
    }

    fn samples_rate(&self) -> u32 {
        // 4096 / 1
        5512
    }

    fn underflowed(&self) -> bool {
        false
    }
}
