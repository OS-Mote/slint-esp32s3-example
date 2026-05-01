#![no_std]
#![no_main]

esp_bootloader_esp_idf::esp_app_desc!();

extern crate alloc;

use bevy::{
    app::App,
    prelude::Update
};
use bevy_platform::time::Instant;
use bevy_ecs::prelude::*;
use slint::platform::software_renderer::MinimalSoftwareWindow;
use esp_backtrace as _;
use esp_alloc::heap_allocator;
use esp_println::println;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::{
    Blocking,
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
            Spi,
            SpiDmaBus
        }
    },
    dma::{
        DmaRxBuf, 
        DmaTxBuf
    },
    dma_buffers
};
use mipidsi::{ 
    NoResetPin, interface::SpiInterface, models::ST7789, options::{
        ColorInversion, Orientation, Rotation
    }
};
use embedded_graphics::{
    prelude::*,
    pixelcolor::Rgb565
};

const DISPLAY_HORIZONTAL_RESOLUTION: usize = 320;
const DISPLAY_VERTICAL_RESOLUTION: usize = 240;
const DISPLAY_BUFFER_SIZE: usize = DISPLAY_HORIZONTAL_RESOLUTION * DISPLAY_VERTICAL_RESOLUTION;

struct Platform {
    window: alloc::rc::Rc<slint::platform::software_renderer::MinimalSoftwareWindow>,
}

impl slint::platform::Platform for Platform {
    fn create_window_adapter(&self) -> Result<alloc::rc::Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }
    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(
            esp_hal::time::Instant::now().duration_since_epoch().as_millis()
        ) 
    }
}

struct WindowResource {
    window: alloc::rc::Rc<MinimalSoftwareWindow>,
}

impl WindowResource {
    fn new() -> Self {
        let window = slint::platform::software_renderer::MinimalSoftwareWindow::new(Default::default());
    
        window.set_size(slint::PhysicalSize::new(DISPLAY_HORIZONTAL_RESOLUTION as u32, DISPLAY_VERTICAL_RESOLUTION as u32));

        WindowResource { window }
    }
}

type Display = mipidsi::Display<
    SpiInterface<
        'static,
        ExclusiveDevice<SpiDmaBus<'static, Blocking>, Output<'static>, Delay>,
        Output<'static>,
    >,
    ST7789,
    NoResetPin,
>;

struct DisplayResource {
    display: Display,
    buffer: alloc::boxed::Box<[slint::platform::software_renderer::Rgb565Pixel; DISPLAY_HORIZONTAL_RESOLUTION]>
}

impl slint::platform::software_renderer::LineBufferProvider for &mut DisplayResource {
    type TargetPixel = slint::platform::software_renderer::Rgb565Pixel;

    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [slint::platform::software_renderer::Rgb565Pixel]),
    ) {
        let rect = embedded_graphics_core::primitives::Rectangle::new(
            Point::new(range.start as _, line as _),
            Size::new(range.len() as _, 1),
        );
        
        render_fn(&mut self.buffer[range.clone()]);

        self.display
            .fill_contiguous(
                &rect,
                self.buffer[range.clone()].iter().map(|p| {
                    embedded_graphics_core::pixelcolor::raw::RawU16::new(p.0).into()
                }),
            )
            .map_err(drop)
            .unwrap();
    }
}

fn render_system(
    window_resource: NonSendMut<WindowResource>,
    display_resource: NonSendMut<DisplayResource>
) {
    window_resource.window.draw_if_needed(|renderer| {
        renderer.render_by_line(display_resource.into_inner());
    });
}

slint::include_modules!();

#[esp_hal::main]
fn main() -> ! {
    esp_alloc::heap_allocator!(size: 1024*32);
    
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
    
    let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) = dma_buffers!(8912);
    let dma_rx_buf = DmaRxBuf::new(rx_descriptors, rx_buffer).unwrap();
    let dma_tx_buf = DmaTxBuf::new(tx_descriptors, tx_buffer).unwrap();

    let spi = Spi::<Blocking>::new(
        peripherals.SPI2, 
        Config::default()
            .with_frequency(Rate::from_mhz(40))
            .with_mode(esp_hal::spi::Mode::_0)
    )
        .unwrap()
        .with_sck(tft_sck)
        .with_mosi(tft_mosi)
        .with_dma(peripherals.DMA_CH0)
        .with_buffers(dma_rx_buf, dma_tx_buf);
    let spi_delay = Delay::new();
    let spi_device: ExclusiveDevice<_, _, Delay> = ExclusiveDevice::new(spi, tft_cs, spi_delay).unwrap();
    let spi_buffer: &mut [u8; 512] = alloc::boxed::Box::leak(alloc::boxed::Box::new([0_u8; 512]));
    let spi_interface = SpiInterface::new(spi_device, tft_dc, spi_buffer);
    let mut display: Display = mipidsi::Builder::new(ST7789, spi_interface)
        .display_size(DISPLAY_VERTICAL_RESOLUTION as u16, DISPLAY_HORIZONTAL_RESOLUTION as u16)
        .invert_colors(ColorInversion::Inverted)
        .orientation(Orientation::default().rotate(Rotation::Deg90))
        .init(&mut Delay::new())
        .unwrap();

    display.clear(Rgb565::BLUE).unwrap();

    let window_resource = WindowResource::new();

    slint::platform::set_platform(alloc::boxed::Box::new(Platform {
        window: window_resource.window.clone()
    }))
    .unwrap();

    let _main_window = MainWindow::new().unwrap();

    // let mut wake_input = Input::new(peripherals.GPIO0, InputConfig::default());

    // wake_input.wakeup_enable(true, esp_hal::gpio::WakeEvent::LowLevel);

    // let mut rtc = Rtc::new(peripherals.LPWR);
    // let mut wake_trigger = WakeTriggers(2);
    // let wake_source = GpioWakeupSource::default();

    // wake_source.apply(&rtc, &mut wake_trigger, &mut RtcSleepConfig(0));

    // rtc.sleep_light(&[&wake_source]);

    fn elapsed_time() -> core::time::Duration {
        core::time::Duration::from_millis(
            esp_hal::time::Instant::now().duration_since_epoch().as_millis()
        )
    }

    unsafe { Instant::set_elapsed(elapsed_time) };

    let mut app = App::new();

    app
        .insert_non_send_resource(window_resource)
        .insert_non_send_resource(DisplayResource { display: display, buffer: alloc::boxed::Box::new([slint::platform::software_renderer::Rgb565Pixel(0); DISPLAY_HORIZONTAL_RESOLUTION]) })
        .add_systems(Update, render_system);

    let loop_delay = Delay::new();
    
    loop {
        slint::platform::update_timers_and_animations();

        app.update();
        
        loop_delay.delay_millis(50u32);
    }

    panic!("The event loop should not return");
}
