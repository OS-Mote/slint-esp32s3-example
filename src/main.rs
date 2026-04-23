#![no_std]
#![cfg_attr(not(feature = "simulator"), no_main)]

extern crate alloc;

slint::include_modules!();
use bevy::prelude::*;


#[cfg(feature = "simulator")]
use { 
    libc_print::std_name::*
};

#[cfg(not(feature = "simulator"))]
use {
    esp_alloc::heap_allocator,
    esp_println::println
};

#[cfg(not(feature = "simulator"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

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
#[esp_hal::main]
fn main() -> ! {
    heap_allocator!(size: 64 * 1024);

    App::new().add_systems(Update, hello_world).run();

    let _ = MainWindow::new().expect("Failed to load UI").run();
    panic!("The event loop should not return");
}

