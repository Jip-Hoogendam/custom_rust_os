use core::fmt::{self};

use bootloader_api::info::FrameBuffer;
use spin::Mutex;

use crate::display::{
    frame_buffer::DisplayBuffer,
    tty::{TTYDisplay, TTYOptions},
};

pub mod frame_buffer;
pub mod tty;

pub static WRITER: Mutex<tty::TTYDisplay<frame_buffer::DisplayBuffer>> =
    Mutex::new(TTYDisplay::unint(DisplayBuffer::uninit()));

pub fn init_writer(framebuffer_info: &'static mut FrameBuffer) {
    WRITER
        .lock()
        .init(DisplayBuffer::from(framebuffer_info), TTYOptions::default());
}

#[doc(hidden)]
pub fn display_print(args: fmt::Arguments) {
    use core::fmt::Write;
    let _ = WRITER.lock().write_fmt(args);
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::display::display_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

//[!tests]
#[test_case]
fn test_println_simple() {
    println!("TEST_PRINT_ONE");
}

#[test_case]
fn test_many_lines() {
    for i in 0..200 {
        println!("TEST_PRINT_MANY: {}", i)
    }
}
