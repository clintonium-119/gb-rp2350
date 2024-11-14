use embedded_hal::digital::{ErrorType, OutputPin, PinState};

#[derive(Default)]
pub struct DummyOutputPin;
impl ErrorType for DummyOutputPin {
    type Error = core::convert::Infallible;
}

impl OutputPin for DummyOutputPin {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn set_state(&mut self, _state: PinState) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[macro_export]
macro_rules! pin_into_function {
    ($pins:expr, $pin_num:expr, $function_type: ty) => {{
        // Generate the appropriate type and function call based on pin number
        paste::paste! {
               $pins.[<gpio $pin_num>].into_function::<$function_type>()
        }
    }};
    ($pins:expr, $pin_num:expr) => {{
        // Generate the appropriate type and function call based on pin number
        paste::paste! {
               $pins.[<gpio $pin_num>].into_function()
        }
    }};
}
