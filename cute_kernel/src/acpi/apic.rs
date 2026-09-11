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
    cell::UnsafeCell,
    mem::MaybeUninit,
    ptr::{read_volatile, write_volatile},
};

use crate::{
    interrupts::interupt_gaurd::{InteruptController, InteruptGaurd},
    memory::addresses::VirtAddr,
    msr::{MSRValues, read_cpu_msr, write_cpu_msr},
    tables::idt::InteruptDiscriptorIndex,
};

/// A module containing all APIC registers.
#[derive(Debug, Clone, Copy)]
#[allow(unused)]
#[repr(u32)]
pub enum APICRegister {
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
    /// This register is used to enable and disable the local APIC it also contains the vector for
    /// spurious inteupts. These can be ignored but should be caught.
    ///
    /// |bit(s)|discription|
    /// |-|-|
    /// |0-7| Spurious interupt vector indexes to the idt. Lower 4 bits(0-3) are hardwired to 1|
    /// |8| Set this to 1 to enable the APIC|
    /// |9| Indicates focus checking. TODO: see what focus checking does|
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
    DivideConfiguration = 0x3E0,
}

#[repr(u8)]
pub enum APICClockDivider {
    Div1 = 0b1011,
    Div2 = 0b0000,
    Div4 = 0b0001,
    Div8 = 0b0010,
    Div16 = 0b0011,
    Div32 = 0b1000,
    Div64 = 0b1001,
    Div128 = 0b1010,
}

#[repr(u32)]
pub enum APICTimerMode {
    Periodic = 0b0010_0000_0000_0000_0000, // Sets the 17th bit
    OneShot = 0,
}

const PIC1_DATA: u16 = 0x21;
const PIC2_DATA: u16 = 0xA1;

const PIC_OFF: u16 = 0xFF;

const IA32_APIC_BASE_MSR: u32 = 0x1B;
const IA32_APIC_BASE_MSR_ENABLE: u32 = 0x800;

#[repr(C, packed(4))]
struct APICRegisters([u32; 256]);

impl APICRegisters {
    #[inline]
    pub fn read(&self, addr: APICRegister) -> u32 {
        unsafe { read_volatile(&raw const self.0[addr as usize / 4]) }
    }

    pub fn write(&mut self, addr: APICRegister, data: u32) {
        unsafe {
            write_volatile(&raw mut self.0[addr as usize / 4], data);
        };
    }
}

pub type ApicId = u32;

/// A structure to work with the local APIC.
///
/// This heavely skirts around rusts safety boundrys. And that is for good reason. Each apic is
/// local to each prosssor and so as long as we garuntee that no task switching happens while
/// changeing the apic(aka disable interupts) then we know for shure that we can safely interact
/// with the apic.
#[repr(C)]
pub struct APIC {
    /// The registers of the apic.
    /// These are all aligned to 16 bytes.
    registers: UnsafeCell<MaybeUninit<&'static mut APICRegisters>>,
}

// Local Apics are only one one cpu. So can be shared across procsesor.
unsafe impl Send for APIC {}
unsafe impl Sync for APIC {}

impl APIC {
    /// # Safety
    /// The apic msut be pointing to a valid apic address
    #[must_use]
    pub const fn uninit() -> Self {
        Self {
            registers: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    #[inline]
    pub fn getid(&self) -> ApicId {
        self.registers().read(APICRegister::APICId)
    }

    #[inline]
    fn set_registers(&self, registers: &'static mut APICRegisters) {
        unsafe { &mut *self.registers.get() }.write(registers);
    }

    #[inline]
    fn registers_mut(&self) -> &mut APICRegisters {
        unsafe { (*self.registers.get()).assume_init_mut() }
    }

    #[inline]
    fn registers(&self) -> &APICRegisters {
        unsafe { (*self.registers.get()).assume_init_ref() }
    }

    #[inline]
    pub fn set_timer_interupt_vector<T: InteruptController>(
        &self,
        vector: InteruptDiscriptorIndex,
        mode: APICTimerMode,
        _interupt_gaurd: &InteruptGaurd<T>,
    ) {
        self.registers_mut()
            .write(APICRegister::LVTTimer, vector as u32 | mode as u32);
    }

    #[inline]
    pub fn set_timer_div<T: InteruptController>(
        &self,
        div: APICClockDivider,
        _interupt_gaurd: &InteruptGaurd<T>,
    ) {
        self.registers_mut()
            .write(APICRegister::DivideConfiguration, div as u32);
    }

    #[inline]
    pub fn set_timer_intial_count<T: InteruptController>(
        &self,
        count: u32,
        _interupt_gaurd: &InteruptGaurd<T>,
    ) {
        self.registers_mut()
            .write(APICRegister::IntialCounterRegister, count);
    }

    #[inline]
    pub fn disable_timer<T: InteruptController>(&self, interupt_gaurd: &InteruptGaurd<T>) {
        self.set_timer_intial_count(0, interupt_gaurd);
    }

    #[inline]
    pub fn end_iterupts<T: InteruptController>(&self, _interupt_gaurd: &InteruptGaurd<T>) {
        self.registers_mut().write(APICRegister::EOI, 0);
    }

    fn ipi_msg(&self, target: u32, command: u32) {
        let registers = self.registers_mut();
        // higher bits first as it sends the command when the lower bits are written to.
        registers.write(APICRegister::InteruptCommand1, target << 24);
        registers.write(APICRegister::InteruptCommand0, command);
    }

    // Ipi code based off https://wiki.osdev.org/Symmetric_Multiprocessing#BSP_Initialization_Code
    pub fn init_ipi(&self, target: u32) {
        const SMI_COMMAND: u32 = 0x00_C500;
        let current = self.registers_mut().read(APICRegister::InteruptCommand0) & 0xFFF0_0000;
        self.ipi_msg(target, current | SMI_COMMAND);
    }

    pub fn deassert(&self, target: u32) {
        const DEASSERT_COMMAND: u32 = 0x00_8500;
        let current = self.registers_mut().read(APICRegister::InteruptCommand0) & 0xFFF0_0000;
        self.ipi_msg(target, current | DEASSERT_COMMAND);
    }

    pub fn await_delivery(&self) {
        const DELIVERY_BIT: u32 = 1 << 12;
        while self.registers_mut().read(APICRegister::InteruptCommand0) & DELIVERY_BIT != 0 {}
    }

    pub fn clear_errors(&self) {
        self.registers_mut().write(APICRegister::ErrorStatus, 0);
    }

    pub fn send_startup_ipi(&self, target: u32, address: u32) {
        const SIPI_COMMAND: u32 = 0x00_0600;
        let current = self.registers_mut().read(APICRegister::InteruptCommand0) & 0xFFF0_F800;
        self.ipi_msg(target, current | SIPI_COMMAND | (address >> 12));
    }

    /// intilizes the APIC
    /// This also disables the 8259 PIC properly just in case it was not disabled.
    ///
    /// # Safety
    /// This function assumes that the APIC is pointing to a correct APIC registers. And this
    /// requires the cpu to suport the Model specific register instructions.
    pub unsafe fn init<T>(&self, addr: VirtAddr, _interupt_gaurd: &InteruptGaurd<T>)
    where
        T: InteruptController,
    {
        use crate::arch::port::Port;
        //Disable pics
        Port::new(PIC1_DATA).write(PIC_OFF);
        Port::new(PIC2_DATA).write(PIC_OFF);

        // Enable the apic in case if it was not enabled.
        let MSRValues { eax, edx } = unsafe { read_cpu_msr(IA32_APIC_BASE_MSR) };
        let base = u64::from(eax & 0xFFFF_F000) | (u64::from(edx & 0x0F) << 32);
        unsafe {
            write_cpu_msr(
                IA32_APIC_BASE_MSR,
                MSRValues {
                    eax: (base & 0xFFFF_F000) as u32 | IA32_APIC_BASE_MSR_ENABLE,
                    edx: 0,
                },
            );
        }

        // sets the apic to the correct registers based off the pointer
        self.set_registers(unsafe { addr.to_mut_ref() });

        // Enable apic by setting the SpuriousInteruptVector.
        self.registers_mut().write(
            APICRegister::SpuriousInteruptVector,
            self.registers().read(APICRegister::SpuriousInteruptVector) | 0x100,
        );
    }
}

pub static LOCAL_APIC: APIC = APIC::uninit();
