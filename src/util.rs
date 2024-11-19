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

pub struct LimitedViewList<'a, T: Sized> {
    list: &'a [T],
    max: usize,
    current_cursor: usize,
}

impl<'a, T> LimitedViewList<'a, T> {
    pub fn new(list: &'a [T], max: usize) -> Self {
        Self {
            list,
            max: usize::min(max, list.len()),
            current_cursor: 0,
        }
    }

    pub fn next(&mut self) {
        if self.current_cursor < self.list.len() {
            self.current_cursor += 1;
        }
    }

    pub fn current_cursor(&self) -> usize {
        self.current_cursor
    }

    pub fn max(&self) -> usize {
        self.max
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn prev(&mut self) {
        if self.current_cursor != 0 {
            self.current_cursor -= 1;
        }
    }

    pub fn iter(&self) -> core::slice::Iter<'a, T> {
        self.list[self.current_cursor..self.current_cursor + self.max].into_iter()
    }
}

#[macro_export]
macro_rules! pin_select {
    ($pins:expr, $pin_num:expr) => {{
        paste::paste! {
               $pins.[<gpio $pin_num>]
        }
    }};
}
