/// A module for interacting with the APIC.
/// APIC's can be used for multiple functions.
///
/// A list of possible things that a APIC can do is as follows :
/// - Setup more accurate timer interupts.
/// - Initlize multiple procssor's.
/// - Inter procssor communication.
/// ## Reading
/// For more explanation about how the APIC works look at eighter:
/// [multiprocessing support for hobby oses explained](http://www.osdever.net/tutorials/view/multiprocessing-support-for-hobby-oses-explained)
/// [os-dev apic](https://wiki.osdev.org/APIC)
use core::{
    arch::x86_64::__cpuid,
    ptr::{read_volatile, write_volatile},
};

use x86_64::instructions::port::Port;

use crate::msr::{MSRValues, read_cpu_msr, write_cpu_msr};

/// A module containing all APIC registers.
#[allow(unused)]
#[repr(u32)]
pub enum APICRegisters {
    /// ## permissions : `Read/Write`.
    /// TODO: Figure out why we have write perms for this.
    APICId = 0x020,

    /// ## permissions : `Read`.
    LapicVersion = 0x30,

    /// This sets the minimum priority of interupt that will trigger.
    ///
    /// ## permissions : `Read/Write`.
    TaskPriority = 0x080,

    /// ## permissions: `Read`.
    AbitrationPriority = 0x090,

    /// ## permissions: `Read`.
    ProcessorPriority = 0x0A0,

    /// ## permissions: `Write`.
    ///
    /// Write zero to this register to indicate the en of the interupt.
    EOI = 0x0B0,

    /// ## permissions: `Read`.
    RemoteRead = 0x0C0,

    /// ## permissions: `Read/Write`.
    LogicalDestination = 0x0D0,

    /// ## permissions: `Read/Write`.
    DestinationFormat = 0x0E0,

    /// # Function
    ///
    /// This register is used to enable and disable the local APIC it also contains
    ///
    /// ## permissions: `Read/Write`.
    SpuriousInteruptVector = 0x0F0,

    /// ## permissions: `Read`.
    InService0 = 0x100,
    InService1 = 0x110,
    InService2 = 0x120,
    InService3 = 0x130,
    InService4 = 0x140,
    InService5 = 0x150,
    InService6 = 0x160,

    /// ## permissions: `Read`.
    TriggerMode0 = 0x180,
    TriggerMode1 = 0x190,
    TriggerMode2 = 0x1A0,
    TriggerMode3 = 0x1B0,
    TriggerMode4 = 0x1C0,
    TriggerMode5 = 0x1D0,
    TriggerMode6 = 0x1E0,
    TriggerMode7 = 0x1F0,

    /// ## permissions: `Read`,
    InteruptRequest0 = 0x200,
    InteruptRequest1 = 0x210,
    InteruptRequest2 = 0x220,
    InteruptRequest3 = 0x230,
    InteruptRequest4 = 0x240,
    InteruptRequest5 = 0x250,
    InteruptRequest6 = 0x260,
    InteruptRequest7 = 0x270,

    /// ## permissions: `Read`.
    ErrorStatus = 0x280,

    /// ## permissions: `Read/Write`.
    LVTCorrectedMachineCheckInterupts = 0x2F0,

    /// ## permissions: `Read/Write`.
    InteruptCommand0 = 0x300,
    InteruptCommand1 = 0x310,

    /// ## permissions: `Read/Write`.
    LVTTimer = 0x320,

    /// ## permissions: `Read/Write`.
    LVTThermalSensor = 0x330,

    /// ## permissions: `Read/Write`.
    LVTPerformanceMonitoringCounters = 0x340,

    /// ## permissions: `Read/Write`.
    LVTLINT0 = 0x350,

    /// ## permissions: `Read/Write`.
    LVTLINT1 = 0x360,

    /// ## permissions: `Read/Write`.
    LVTError = 0x370,

    /// ## permissions: `Read/Write`.
    IntialCounterRegister = 0x380,

    /// ## permissions: `Read`.
    CurrentCountRegister = 0x390,

    /// ## permissions: `Read/Write`
    DeviceConfiguration = 0x3E0,
}

const PIC1_DATA: u16 = 0x21;
const PIC2_DATA: u16 = 0xA1;

const PIC_OFF: u16 = 0xFF;
const APIC_CPUID_OFFSET: u32 = 1 << 9;

const IA32_APIC_BASE_MSR: u32 = 0x1B;
const IA32_APIC_BASE_MSR_ENABLE: u32 = 0x800;

/// A structure to work with the local APIC.
///

#[derive(Debug)]
#[repr(C, packed)]
pub struct APIC {
    /// The registers of the apic.
    /// These are all aligned to 16 bytes.
    registers: [u8; 0x3F0],
}

impl APIC {
    /// Writes to a APIC register.
    #[inline]
    fn read_addr(&self, addr: APICRegisters) -> u32 {
        let pointer = core::ptr::from_ref::<Self>(self);
        #[allow(clippy::cast_ptr_alignment)]
        unsafe {
            read_volatile(pointer.add(addr as usize).cast::<u32>())
        }
    }

    /// Read a register from a APIC.
    #[inline]
    fn write_addr(&mut self, addr: APICRegisters, data: u32) {
        let pointer = core::ptr::from_mut::<Self>(self);
        #[allow(clippy::cast_ptr_alignment)]
        unsafe {
            write_volatile(pointer.add(addr as usize).cast::<u32>(), data);
        };
    }

    /// intilizes the APIC
    /// This also disables the 8259 PIC properly just in case it was not disabled.
    ///
    /// # Safety
    /// This function assumes that the APIC is pointing to a correct APIC registers. And this
    /// requires the cpu to suport the Model specific register instructions.
    pub unsafe fn init(&mut self) {
        let cpuid = __cpuid(1);
        debug_assert!(
            cpuid.edx & APIC_CPUID_OFFSET != 0,
            "cpu does not provide a APIC"
        );

        //Disable pics
        unsafe {
            Port::new(PIC1_DATA).write(PIC_OFF);
            Port::new(PIC2_DATA).write(PIC_OFF);
        }

        // Enable the apic in case if it was not enabled.
        let MSRValues { eax, edx } = unsafe { read_cpu_msr(IA32_APIC_BASE_MSR) };
        let base = u64::from(eax & 0xFFFF_F000) | (u64::from(edx & 0xF) << 32);
        unsafe {
            write_cpu_msr(
                IA32_APIC_BASE_MSR,
                MSRValues {
                    eax: (base & 0xFFFF_F000) as u32 | IA32_APIC_BASE_MSR_ENABLE,
                    edx: 0,
                },
            );
        }

        // set the SpuriousInteruptVector.
    }
}
