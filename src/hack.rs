

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::OutputPin;
use embedded_hal::spi::{ErrorType, Operation, SpiBus};
use rp235x_hal::gpio::bank0::Gpio5;
use rp235x_hal::gpio::{FunctionSio, Pin, PullDown, SioOutput};

pub struct SpiWithCS<S: SpiBus> {
    pub bus: S,
    pub cs: Pin<Gpio5, FunctionSio<SioOutput>, PullDown>
    // TODO: Delay (not actually _used_ by the ili crate but can't hurt to pass it in)
}

impl<S: SpiBus> ErrorType for SpiWithCS<S> { type Error = S::Error; }

impl<S: SpiBus> embedded_hal::spi::SpiDevice for SpiWithCS<S> {
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        self.cs.set_low().unwrap();
        for op in operations {
            match op {
                Operation::Read(buf) => self.bus.read(buf)?,
                Operation::Write(buf) => self.bus.write(buf)?,
                Operation::Transfer(rd, wr) => self.bus.transfer(rd, wr)?,
                Operation::TransferInPlace(buf) => self.bus.transfer_in_place(buf)?,
                Operation::DelayNs(_) => () // TODO: Delay
            }
        }
        self.cs.set_high().unwrap();
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        self.cs.set_low().unwrap();
        self.bus.read(buf)?;
        self.cs.set_high().unwrap();
        Ok(())
    }

    fn write(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.cs.set_low().unwrap();
        self.bus.write(buf)?;
        self.cs.set_high().unwrap();
        Ok(())
    }

    fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Self::Error> {
        self.cs.set_low().unwrap();
        self.bus.transfer(read, write)?;
        self.cs.set_high().unwrap();
        Ok(())
    }

    fn transfer_in_place(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        self.cs.set_low().unwrap();
        self.bus.transfer_in_place(buf)?;
        self.cs.set_high().unwrap();
        Ok(())
    }
}