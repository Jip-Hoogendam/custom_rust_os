use core::arch::x86_64::__cpuid;

use x86_64::instructions::port::Port;

use crate::msr::{self, MSRValues, read_cpu_msr, write_cpu_msr};

const PIC1_DATA: u16 = 0x21;
const PIC2_DATA: u16 = 0xA1;
const PIC_OFF: u16 = 0xFF;
const APIC_CPUID_OFFSET: u32 = 1 << 9;

const IA32_APIC_BASE_MSR: u32 = 0x1B;
const IA32_APIC_BASE_MSR_ENABLE: u32 = 0x800;

struct AIPC {
    base: u64,
}

/// This implemantation is highly based off the https://wiki.osdev.org/APIC explenation
impl AIPC {
    /// Inits this threads APIC
    ///
    /// # Warning
    /// This function assumes that the prosessor has a APIC
    /// if the prosessor does not have a apic it will unwind instead
    pub fn init() {
        let cpuid = __cpuid(1);
        if cpuid.edx & APIC_CPUID_OFFSET == 0 {
            panic!("cpu does not provide a APIC");
        }

        //Disable pics
        unsafe {
            Port::new(PIC1_DATA).write(PIC_OFF);
            Port::new(PIC2_DATA).write(PIC_OFF);
        }

        let MSRValues { eax, edx } = unsafe { read_cpu_msr(IA32_APIC_BASE_MSR) };
        let base = (eax & 0xFFFFF000) as u64 | (((edx & 0xF) as u64) << 32);
        unsafe {
            write_cpu_msr(
                IA32_APIC_BASE_MSR,
                MSRValues {
                    eax: (base & 0xFFFFF000) as u32 | IA32_APIC_BASE_MSR_ENABLE,
                    edx: 0,
                },
            );
        }
    }
}
