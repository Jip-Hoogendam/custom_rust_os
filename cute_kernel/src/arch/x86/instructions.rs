use core::arch::asm;

pub fn hlt() {
    unsafe {
        asm!("hlt", options(nomem, nostack, preserves_flags));
    }
}

pub fn int3() {
    unsafe {
        asm!("int3", options(nomem, nostack, preserves_flags));
    }
}
