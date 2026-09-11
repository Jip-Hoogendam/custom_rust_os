use core::{
    mem::MaybeUninit,
    ptr::{read_volatile, write_volatile},
};

use crate::{
    acpi::apic::ApicId, memory::addresses::VirtAddr, println, sync::Mutex,
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

    fn write_register(&mut self, register: u32, data: u32) {
        unsafe {
            write_volatile(&raw mut **self.io_reg_sel.assume_init_mut(), register);
            write_volatile(&raw mut **self.io_win.assume_init_mut(), data);
        }
    }

    fn read_register(&mut self, register: u32) -> u32 {
        unsafe {
            write_volatile(&raw mut **self.io_reg_sel.assume_init_mut(), register);
            read_volatile(&raw const **self.io_win.assume_init_mut())
        }
    }

    fn len(&mut self) -> u32 {
        const VERSION_OFFSET: u32 = 0x1;
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

    #[inline]
    pub fn set_irq_physical(
        &mut self,
        irq: IRQ,
        vector: InteruptDiscriptorIndex,
        desitnation: ApicId,
    ) {
        const IOREDBL_OFFSET: u32 = 0x10;
        const DESTINATION_OFFSET: usize = 56;
        debug_assert!(self.is_responsable(irq));

        let data = vector as u64 | u64::from(desitnation) << DESTINATION_OFFSET;
        let register = (irq as u32 - self.global_system_interupt_base) * 2 + IOREDBL_OFFSET;
        #[allow(clippy::cast_possible_truncation)]
        self.write_register(register, data as u32);
        self.write_register(register + 1, (data >> 32) as u32);
    }
}

/// FIXME: `ScanMDAT` for IRQ overides and multiple `IoApic`'s
pub static IOAPIC: Mutex<IoApic> = Mutex::new(IoApic::uninit());
