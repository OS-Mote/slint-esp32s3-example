#![no_std]
#![cfg_attr(not(feature = "simulator"), no_main)]

esp_bootloader_esp_idf::esp_app_desc!();

extern crate alloc;

slint::include_modules!();

#[cfg(not(feature = "simulator"))]
// use core::convert::Infallible;

use bevy::app::{App, ScheduleRunnerPlugin, Startup, TaskPoolPlugin};
use bevy::prelude::Update;
use bevy::time::TimePlugin;
use bevy_ecs::prelude::*;

#[cfg(feature = "simulator")]
use { 
    libc_print::std_name::*
};

#[cfg(not(feature = "simulator"))]
use {
    esp_backtrace as _,
    esp_alloc::heap_allocator,
    esp_println::println,
    embedded_hal_bus::spi::{
        ExclusiveDevice,
    },
    embedded_hal::{
        digital::OutputPin,
    },
    esp_hal::{
        delay::Delay,
        gpio::{ 
            Level,
            Output, 
            OutputConfig,
        },
        rtc_cntl::{
            Rtc, 
            sleep::{
                RtcSleepConfig,
                GpioWakeupSource,
                WakeSource,
                WakeTriggers
            }
        },
        time::Rate,
        spi::{
            master::{ 
                Config, 
                Spi
            }
        }
    },
    mipidsi::{ 
        interface::{
            SpiInterface,
        },
        models::ST7789, 
        options::{
            ColorInversion,
            Rotation,
            Orientation
        }
    },
    embedded_graphics::{
        prelude::*,
        pixelcolor::Rgb565
    }
};



#[cfg(not(feature = "simulator"))]

fn hello_world() {
    println!("hello world!");
}

#[cfg(feature = "simulator")]
fn main() -> Result<(), slint::PlatformError> {
    App::new().add_systems(Update, hello_world).run();

    let _ = MainWindow::new().expect("Failed to load UI").run();
    panic!("The event loop should not return");
}

#[cfg(not(feature = "simulator"))]
struct SlintPlatform {
    window: alloc::rc::Rc<slint::platform::software_renderer::MinimalSoftwareWindow>,
}

impl slint::platform::Platform for SlintPlatform {
    fn create_window_adapter(
        &self,
    ) -> Result<alloc::rc::Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError>
    {
        Ok(self.window.clone())
    }

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(
            esp_hal::time::Instant::now().duration_since_epoch().as_millis()
        ) 
    }

    fn debug_log(&self, arguments: core::fmt::Arguments) {
        esp_println::println!("{}", arguments);
    }
}

#[cfg(not(feature = "simulator"))]
#[esp_hal::main]
fn main() -> ! {
    esp_alloc::heap_allocator!(size: 64000);
    
    let peripherals = esp_hal::init(esp_hal::Config::default());

    let mut backlight = Output::new(peripherals.GPIO21, Level::High, OutputConfig::default());
    
    backlight.set_high();

    let mut tft_cs = Output::new(peripherals.GPIO41, Level::High, OutputConfig::default());

    tft_cs.set_high();

    let tft_sck = peripherals.GPIO11;
    let tft_mosi = peripherals.GPIO9;
    let tft_dc = Output::new(peripherals.GPIO16, Level::Low, OutputConfig::default());
    let mut tft_enable = Output::new(peripherals.GPIO42, Level::High, OutputConfig::default());
    
    tft_enable.set_high();
    
    let spi = Spi::new(peripherals.SPI2, Config::default().with_frequency(Rate::from_mhz(40)))
    .unwrap()
    .with_sck(tft_sck)
    .with_mosi(tft_mosi);

    let mut buffer = [0u8; 512];

    let spi_delay = Delay::new();
    let spi_device = ExclusiveDevice::new(spi, tft_cs, spi_delay).unwrap();
    let di = SpiInterface::new(spi_device, tft_dc, &mut buffer);
    
    let mut display_delay = Delay::new();
    
    let mut display = mipidsi::Builder::new(ST7789, di)
        .display_size(240, 320)
        .invert_colors(ColorInversion::Inverted)
        .init(&mut display_delay)
        .unwrap();

    display.set_orientation(Orientation::default().rotate(Rotation::Deg90)).unwrap();

    display.clear(Rgb565::BLUE).unwrap();

    let window = slint::platform::software_renderer::MinimalSoftwareWindow::new(Default::default());
    
    window.set_size(slint::PhysicalSize::new(320, 240));
    
    slint::platform::set_platform(alloc::boxed::Box::new(MyPlatform {
        window: window.clone()
    }))
    .unwrap();

    struct MyPlatform {
        window: alloc::rc::Rc<slint::platform::software_renderer::MinimalSoftwareWindow>,
    }

    impl slint::platform::Platform for MyPlatform {
        fn create_window_adapter(&self) -> Result<alloc::rc::Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
            Ok(self.window.clone())
        }
        fn duration_since_start(&self) -> core::time::Duration {
            core::time::Duration::from_millis(
                esp_hal::time::Instant::now().duration_since_epoch().as_millis()
            ) 
        }
    }

    let main_window = MainWindow::new().unwrap();

    let mut line = [slint::platform::software_renderer::Rgb565Pixel(0); 320];

    // let mut wake_input = Input::new(peripherals.GPIO0, InputConfig::default());

    // wake_input.wakeup_enable(true, esp_hal::gpio::WakeEvent::LowLevel);

    // let mut rtc = Rtc::new(peripherals.LPWR);
    // let mut wake_trigger = WakeTriggers(2);
    // let wake_source = GpioWakeupSource::default();

    // wake_source.apply(&rtc, &mut wake_trigger, &mut RtcSleepConfig(0));

    // rtc.sleep_light(&[&wake_source]);

    loop {
        slint::platform::update_timers_and_animations();

        window.draw_if_needed(|renderer| {
            use embedded_graphics_core::prelude::*;
            struct DisplayWrapper<'a, T>(
                &'a mut T,
                &'a mut [slint::platform::software_renderer::Rgb565Pixel],
            );
            impl<T: DrawTarget<Color = embedded_graphics_core::pixelcolor::Rgb565>>
                slint::platform::software_renderer::LineBufferProvider for DisplayWrapper<'_, T>
            {
                type TargetPixel = slint::platform::software_renderer::Rgb565Pixel;
                fn process_line(
                    &mut self,
                    line: usize,
                    range: core::ops::Range<usize>,
                    render_fn: impl FnOnce(&mut [Self::TargetPixel]),
                ) {
                    let rect = embedded_graphics_core::primitives::Rectangle::new(
                        Point::new(range.start as _, line as _),
                        Size::new(range.len() as _, 1),
                    );
                    render_fn(&mut self.1[range.clone()]);
                    // NOTE! this is not an efficient way to send pixel to the screen, but it is kept simple on this template.
                    // It would be much faster to use the DMA to send pixel in parallel.
                    // See the example in https://github.com/slint-ui/slint/blob/master/examples/mcu-board-support/pico_st7789.rs 
                    self.0
                        .fill_contiguous(
                            &rect,
                            self.1[range.clone()].iter().map(|p| {
                                embedded_graphics_core::pixelcolor::raw::RawU16::new(p.0).into()
                            }),
                        )
                        .map_err(drop)
                        .unwrap();
                }
            }
            renderer.render_by_line(DisplayWrapper(&mut display, &mut line));
        });

        if window.has_active_animations() {
            continue;
        }
    }

    panic!("The event loop should not return");
}
