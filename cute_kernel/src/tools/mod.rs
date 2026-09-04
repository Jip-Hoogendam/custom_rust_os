use core::arch::asm;

use crate::println;

/// gets the current cpu id.
#[inline]
#[must_use]
pub fn cpu_id() -> u16 {
    let mut id = 0u32;
    unsafe {
        asm!(
        r#"
            mov eax, 1
            cpuid
            mov rax, rbx
        "#,
            out("rax") id,
            options(nostack)
        );
    }
    (id >> 24) as u16
}
