use crate::interrupts::notfiy_end_of_timer_interrupt;

use super::InterruptStackFrame;
/// Keyboard interupts should get called on each keyboard signal.
pub extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    crate::tasks::keyboard::add_scancode(scancode);

    notfiy_end_of_timer_interrupt();
}
