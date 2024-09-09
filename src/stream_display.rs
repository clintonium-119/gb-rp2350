use embedded_hal::digital::OutputPin;

use crate::rp_hal::hal;
use hal::dma::SingleChannel;
use hal::pio::{PIOExt, StateMachineIndex, UninitStateMachine, PIO};

use crate::array_scaler::ScreenHandler;
use crate::pio_dma_interface;

pub struct Streamer<CH> {
    dma_channel: Option<CH>,
    spare_buffer: Option<&'static mut [u16]>,
    main_buffer: Option<&'static mut [u16]>,
    clock_freq: u16,
    rw: u8,
    pins: (u8, u8),
}

impl<CH> Streamer<CH>
where
    CH: SingleChannel,
{
    pub fn new(
        clock_freq: u16,
        rw: u8,
        pins: (u8, u8),
        channel: CH,
        spare_buffer: &'static mut [u16],
        main_buffer: &'static mut [u16],
    ) -> Self {
        Self {
            dma_channel: Some(channel),
            spare_buffer: Some(spare_buffer),
            main_buffer: Some(main_buffer),
            clock_freq,
            rw,
            pins,
        }
    }

    pub fn stream<const SCREEN_WIDTH: usize, P, RS, SM, I>(
        &mut self,
        pio: &mut PIO<P>,
        rs: RS,
        sm: UninitStateMachine<(P, SM)>,
        iterator: &mut I,
    ) -> (RS, UninitStateMachine<(P, SM)>)
    where
        P: PIOExt,
        RS: OutputPin,
        SM: StateMachineIndex,
        I: Iterator<Item = u16>,
    {
        let channel = core::mem::replace(&mut self.dma_channel, None).unwrap();
        let spare_buffer = core::mem::replace(&mut self.spare_buffer, None).unwrap();
        let main_buffer = core::mem::replace(&mut self.main_buffer, None).unwrap();

        let stream = pio_dma_interface::PioInterface::new(
            self.clock_freq,
            rs,
            pio,
            sm,
            self.rw,
            self.pins,
            main_buffer,
            channel,
        );

        let sh: ScreenHandler<SCREEN_WIDTH, _, _> =
            ScreenHandler::new(iterator, stream, spare_buffer);
        let (stream, spare_buffer) = sh.compute_line();

        let (channel, sm, main_buffer, rs) = stream.free(pio);

        self.main_buffer = Some(main_buffer);
        self.spare_buffer = Some(spare_buffer);
        self.dma_channel = Some(channel);

        (rs, sm)
    }
}
