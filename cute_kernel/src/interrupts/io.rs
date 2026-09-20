use crate::{
    acpi::apic::LOCAL_APIC,
    interrupts::interupt_gaurd::{InteruptAlreadyBlocked, InteruptGaurd},
};

use super::ExceptionStackFrame;
/// Keyboard interupts should get called on each keyboard signal.
pub extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: ExceptionStackFrame) {
    use crate::arch::port::Port;

    let port = Port::new(0x60);
    let scancode: u8 = port.read();
    crate::tasks::keyboard::add_scancode(scancode);

    let interupt_gaurd = InteruptGaurd::<InteruptAlreadyBlocked>::block();
    LOCAL_APIC.end_iterupts(&interupt_gaurd);
}

pub extern "x86-interrupt" fn system_clock_tick(_stack_frame: ExceptionStackFrame) {
    unsafe {
        crate::time::SYSTEM_CLOCK.tick();
        let interupt_gaurd = InteruptGaurd::<InteruptAlreadyBlocked>::block();
        LOCAL_APIC.end_iterupts(&interupt_gaurd);
    }
}
