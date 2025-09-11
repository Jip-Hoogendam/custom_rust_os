
#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(owos_prime::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use owos_prime::{halt_loop, println};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    owos_prime::init();

    #[cfg(test)]
    test_main();

    halt_loop();
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    halt_loop();
}


//this funciton is called on panics in tests
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    owos_prime::test_panic_handler(info)
}

