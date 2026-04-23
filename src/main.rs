#![no_std]
#![cfg_attr(not(feature = "simulator"), no_main)]

extern crate alloc;

slint::include_modules!();

// use esp_alloc::heap_allocator;

// #[cfg(not(feature = "simulator"))]
// #[hal::entry]

#[cfg(not(feature = "simulator"))]
use {
    esp_alloc::heap_allocator,
    esp_backtrace as _,
};

#[cfg(feature = "simulator")]
fn main() -> Result<(), slint::PlatformError> {
    let _ = MainWindow::new().expect("Failed to load UI").run();
    panic!("The event loop should not return");
}

#[cfg(not(feature = "simulator"))]
#[esp_hal::main]
fn main() -> ! {
    heap_allocator!(size: 64 * 1024);

    let _ = MainWindow::new().expect("Failed to load UI").run();
    panic!("The event loop should not return");
}
