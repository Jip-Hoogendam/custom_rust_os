use core::arch::asm;

/// A structue to set or read MSR values.
pub struct MSRValues {
    pub eax: u32,
    pub edx: u32,
}

/// Reads the cpu MSR register.
///
/// # Safety
/// The cpu should be have a msr before this function gets called.
#[inline]
pub unsafe fn read_cpu_msr(msr: u32) -> MSRValues {
    let eax: u32;
    let edx: u32;
    unsafe {
        asm!(
        "rdmsr",  in("ecx") msr, out("eax") eax, out("edx") edx)
    }

    MSRValues { eax, edx }
}

/// Writes to the cpu MSR register
///
/// # Safety
/// The cpu should be have a msr before this function gets called.
#[inline]
pub unsafe fn write_cpu_msr(msr: u32, MSRValues { eax, edx }: MSRValues) {
    unsafe {
        asm!("wrmsr",in("ecx") msr,in("eax") eax,in("edx") edx);
    }
}
