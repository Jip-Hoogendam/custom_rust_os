use core::arch::asm;

pub type CPUId = u16;

/// gets the current cpu id.
#[inline]
#[must_use]
pub fn cpu_id() -> CPUId {
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
    (id >> 24) as CPUId
}
