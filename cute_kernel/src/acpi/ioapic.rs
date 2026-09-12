use core::{
    mem::MaybeUninit,
    ops::Add,
    ptr::{read_volatile, write_volatile},
};

use crate::{
    acpi::apic::ApicId, memory::addresses::VirtAddr, sync::Mutex,
    tables::idt::InteruptDiscriptorIndex,
};

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum IRQ {
    PIT = 0,
    Keyboard = 1,
    Cascade = 2,
    COM1 = 3,
    COM2 = 4,
    LPT2 = 5,
    FloppyDisk = 6,
    LPT1 = 7,
    CMOSRtc = 8,
    FreeForPeriphierals1 = 9,
    FreeForPeriphierals2 = 10,
    FreeForPeriphierals3 = 11,
    PS2Mouse = 12,
    FPU = 13,
    ATA1 = 14,
    ATA2 = 15,
}

pub type IOApicId = u32;

/// A struct to make the apic register type safe
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
struct ApicRegister(u32);

impl Add<u32> for ApicRegister {
    type Output = Self;
    fn add(self, rhs: u32) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl ApicRegister {
    const IOREDBL_OFFSET: u32 = 0x10;

    #[inline]
    #[must_use]
    pub const fn new(register: u32) -> Self {
        Self(register)
    }

    #[inline]
    #[must_use]
    pub const fn new_irq(register: u32) -> Self {
        Self::new(register * 2 + Self::IOREDBL_OFFSET)
    }
}

/// A interface to interact with the ioapic
pub struct IoApic {
    global_system_interupt_base: u32,
    io_reg_sel: MaybeUninit<&'static mut u32>,
    io_win: MaybeUninit<&'static mut u32>,
}

impl IoApic {
    /// # Safety
    /// The caller mst garuntee that the passed address is to a actual `IoApicRegisters`
    #[inline]
    pub const unsafe fn init(&mut self, addr: VirtAddr, base: u32) {
        self.global_system_interupt_base = base;
        unsafe {
            self.io_reg_sel.write(addr.to_mut_ref());
            self.io_win.write(&mut *(addr.to_mut_ptr::<u32>()).add(4));
        }
    }

    #[inline]
    fn write_register(&mut self, register: ApicRegister, data: u32) {
        unsafe {
            write_volatile(&raw mut **self.io_reg_sel.assume_init_mut(), register.0);
            write_volatile(&raw mut **self.io_win.assume_init_mut(), data);
        }
    }

    #[inline]
    #[must_use]
    fn read_register(&mut self, register: ApicRegister) -> u32 {
        unsafe {
            write_volatile(&raw mut **self.io_reg_sel.assume_init_mut(), register.0);
            read_volatile(&raw const **self.io_win.assume_init_mut())
        }
    }

    fn len(&mut self) -> u32 {
        const VERSION_OFFSET: ApicRegister = ApicRegister::new(0x1);
        const LEN_BITS: usize = 16;
        (self.read_register(VERSION_OFFSET) >> LEN_BITS) + 1
    }

    #[must_use]
    pub const fn uninit() -> Self {
        Self {
            global_system_interupt_base: 0,
            io_reg_sel: MaybeUninit::uninit(),
            io_win: MaybeUninit::uninit(),
        }
    }

    #[inline]
    #[must_use]
    pub fn is_responsable(&mut self, irq: IRQ) -> bool {
        irq as u32 - self.global_system_interupt_base < self.len()
    }

    /// Returns the register that is used for this interupt. Returning nothing if this apic is not
    /// responsable.
    #[inline]
    #[must_use]
    fn register(&mut self, irq: IRQ) -> Option<ApicRegister> {
        let irq_offset = irq as u32 - self.global_system_interupt_base;
        if irq_offset < self.len() {
            Some(ApicRegister::new_irq(irq_offset))
        } else {
            None
        }
    }

    const DESTINATION_OFFSET: usize = 56;

    /// # Panics
    /// if the io apic is not responsable for this.
    #[inline]
    pub fn set_irq_physical(
        &mut self,
        irq: IRQ,
        vector: InteruptDiscriptorIndex,
        desitnation: ApicId,
    ) {
        let data = vector as u64 | u64::from(desitnation) << Self::DESTINATION_OFFSET;
        let register = self.register(irq).unwrap();
        #[allow(clippy::cast_possible_truncation)]
        self.write_register(register, data as u32);
        self.write_register(register + 1, (data >> 32) as u32);
    }
}

/// FIXME: `ScanMDAT` for IRQ overides and multiple `IoApic`'s
pub static IOAPIC: Mutex<IoApic> = Mutex::new(IoApic::uninit());
