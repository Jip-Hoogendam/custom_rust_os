use core::arch::asm;

pub struct MSRValues {
    pub eax: u32,
    pub edx: u32,
}

/// Reads the cpu MSR register.
///
/// # Safety
/// assumes that the prosessor has a msr.
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
/// Assumes that the prosessor has a msr.
pub unsafe fn write_cpu_msr(msr: u32, MSRValues { eax, edx }: MSRValues) {
    unsafe {
        asm!("wdmsr",in("ecx") msr,in("eax") eax,in("edx") edx);
    }
}
