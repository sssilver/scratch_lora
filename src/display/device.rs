use embedded_graphics::{
    draw_target::DrawTarget,
    mono_font::{iso_8859_1::FONT_6X10, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::Point,
    text::{Baseline, Text},
    Drawable,
};
use esp_hal::{delay::Delay, gpio::Output, i2c::master::I2c, Async};
use ssd1306::{
    mode::{BufferedGraphicsMode, DisplayConfig},
    prelude::*,
    size::DisplaySize128x64,
    I2CDisplayInterface, Ssd1306,
};

#[derive(Debug)]
pub enum DisplayInitError {
    Reset,
    Init,
    Flush,
}

pub struct DisplayDevice<'a> {
    display: Ssd1306<
        I2CInterface<I2c<'a, Async>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
    // oled_rst: Output<'a>,
}

impl<'a> DisplayDevice<'a> {
    /// Create a new Display instance
    pub fn new(
        i2c: I2c<'a, Async>,
        mut oled_rst: Output<'a>,
        delay: &mut Delay,
    ) -> Result<Self, DisplayInitError> {
        let i2c_display_interface = I2CDisplayInterface::new_custom_address(i2c, 0x3C);

        let mut display = Ssd1306::new(
            i2c_display_interface,
            DisplaySize128x64,
            DisplayRotation::Rotate0,
        )
        .into_buffered_graphics_mode();

        display
            .reset(&mut oled_rst, delay)
            .map_err(|_| DisplayInitError::Reset)?;

        display.init().map_err(|_| DisplayInitError::Init)?;

        Ok(Self {
            display, /*, oled_rst */
        })
    }

    /// Clear the display
    pub fn clear(&mut self) -> Result<(), DisplayInitError> {
        self.display.clear(BinaryColor::Off).unwrap();
        self.display.flush().map_err(|_| DisplayInitError::Flush)?;

        Ok(())
    }

    pub fn draw_text(&mut self, text: &str, position: Point) -> Result<(), DisplayInitError> {
        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::On)
            .build();

        Text::with_baseline(text, position, text_style, Baseline::Top)
            .draw(&mut self.display)
            .unwrap();

        defmt::info!("Drawing: {}", text);

        self.display.flush().unwrap();

        Ok(())
    }
}
