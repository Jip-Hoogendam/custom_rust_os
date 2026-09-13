/// A module to read the Multiple APIC Description Table.
use crate::{
    acpi::{
        ioapic::{IRQ, IRQConfig, IoApic},
        sdt::ShortSDTHeader,
    },
    interrupts::interupt_gaurd::{InteruptController, InteruptGaurd},
    memory::addresses::{PhysAddr, PhysMemOffset},
    println,
    sync::MutexGuard,
};

use super::apic::APIC;

/// The struct for the MADT data header
#[derive(Debug)]
#[repr(C, packed)]
pub struct MADT {
    pub header: ShortSDTHeader,
    pub local_apic_addr: u32,
    pub flags: u32,
}

impl MADT {
    /// Searches for the apic and initlizes it.
    pub fn init_apic<T: InteruptController>(
        &self,
        offset: PhysMemOffset,
        apic: &APIC,
        interupt_gaurd: &InteruptGaurd<T>,
    ) {
        let mut pointer = PhysAddr::from(self.local_apic_addr);

        // properly overide the record.
        for record in self.records() {
            if let record::MADTRecord::LocalAPICOveride(addr) = record {
                pointer = PhysAddr::from(addr.address);
                break;
            }
        }

        unsafe { apic.init(pointer + offset, interupt_gaurd) }
    }

    /// # Panics
    /// Panics if the `IoApic` can not be found
    /// FIXME: allow mutliple IOAPICS
    pub fn init_io_apic(&self, offset: PhysMemOffset, mut io_apic: MutexGuard<'_, IoApic>) {
        for record in self.records() {
            if let record::MADTRecord::IOAPIC(ioapic) = record {
                let addr = PhysAddr::from(ioapic.address) + offset;
                unsafe {
                    io_apic.init(addr, ioapic.global_interupt_system_base);
                }
                return;
            }
        }
        panic!("No IO apic found");
    }

    /// Creates a iterator of all the attached records
    #[must_use]
    pub const fn records(&self) -> MADTRecordIterator {
        let pointer = core::ptr::from_ref::<Self>(self);
        // subtract 4 of the total length due to the record missing it signature
        let end = unsafe { pointer.cast::<u8>().add(self.header.length as usize - 4) };
        let pointer = unsafe { pointer.add(1) }.cast::<u8>();
        MADTRecordIterator { pointer, end }
    }

    #[must_use]
    pub fn get_irq_options(&self, irq: IRQ) -> IRQConfig {
        for record in self.records() {
            if let record::MADTRecord::IOAPICInteruptSourceOveride(overide) = record
                && overide.irq_src == irq
            {
                return IRQConfig::from_overide(overide);
            }
        }
        IRQConfig::new(irq)
    }
}

pub mod record {
    use crate::acpi::ioapic::{GlobalSystemInterupt, IRQ};

    /// The format of the mdat record header,
    #[repr(C, packed)]
    pub struct MADTRecordHeader {
        pub kind: u8,
        pub record_length: u8,
    }

    /// A record denoting a local apic of the processor.
    /// This can be used to setup Symmetric `MultiProssessing` on the prosessor.
    #[derive(Debug)]
    #[repr(C, packed)]
    pub struct LocalAPIC {
        pub record_length: u8,
        /// The id of the ACPI processor.
        pub acpi_processor_id: u8,
        /// The id of the APIC.
        pub id: u8,
        /// Info about the processor.
        /// |bit(s)|discription|
        /// |-|-|
        /// |0| Weather this processor is enabled or not|
        /// |1| If this processor is even capable of being enabled|
        /// |2-31| Reserved|
        pub flags: u32,
    }

    impl LocalAPIC {
        #[must_use]
        pub const fn can_be_used(&self) -> bool {
            const APIC_CAN_IS_ENABLED: u32 = 0b0000_0001;
            const APIC_CAN_BE_ENABLED: u32 = 0b0000_0010;

            self.flags & APIC_CAN_BE_ENABLED != 0 || self.flags & APIC_CAN_IS_ENABLED != 0
        }
    }

    /// A record refrenceing a I/O APIC in on the processor
    /// This is required to setup Symmetric `MultiProssessing`.
    #[derive(Debug)]
    #[repr(C, packed)]
    pub struct IOAPIC {
        pub record_length: u8,
        /// The id of the I/O APIC.
        pub id: u8,
        /// Reseverd data idk eighter.
        pub reserved: u8,
        /// The adress of the I/O APIC
        pub address: u32,
        /// The fist interupt number that this I/O APIC handles.
        pub global_interupt_system_base: u32,
    }

    /// A record of the I/O APIC source overide.
    /// This is used to explain how the IRQ is mapped to the global system interupts.
    /// TODO: Firgure out how this works
    #[derive(Debug)]
    #[repr(C, packed)]
    pub struct IOAPICInteruptSourceOveride {
        pub record_length: u8,
        pub bus_src: u8,
        pub irq_src: IRQ,
        pub global_system_interupt: GlobalSystemInterupt,
        pub flags: u16,
    }

    /// A record denoting a non maskable interupt source on the I/O APIC.
    /// TODO: figure out how this works
    #[derive(Debug)]
    #[repr(C, packed)]
    pub struct IOAPICNonMaskableInteruptSource {
        pub record_length: u8,
        pub nmi_source: u8,
        pub reserved: u8,
        pub flags: u16,
        pub global_system_interupt: u32,
    }

    /// A record denoting a non maskable interupt source on the local APIC.
    /// TODO: Figure out how this works
    #[derive(Debug)]
    #[repr(C, packed)]
    pub struct LocalAPICNonMaskableInteruptSource {
        pub record_length: u8,
        pub acpi_processor_id: u8,
        pub flags: u16,
        pub lint: u8,
    }

    /// A record denoting the overide of the local APIC register for long mode cpus.
    ///
    /// Only one record of this can be present and if it is present this should be used insted of
    /// the one specified in the MADT.
    #[derive(Debug)]
    #[repr(C, packed)]
    pub struct LocalAPICOveride {
        pub record_length: u8,
        pub reserved: u16,
        /// The new address that should be used
        pub address: u64,
    }

    /// A record denoting the a prosessors local x2APIC.
    /// TODO: Figure out what a x2APIC is and how to use it.
    #[derive(Debug)]
    #[repr(C, packed)]
    pub struct LocalAPIC2X {
        pub record_length: u8,
        pub reserved: u16,
        pub id: u32,
        pub flags: u32,
        pub acpi_id: u32,
    }

    /// A collector structure of all the records.
    /// This is handy to differentiate between diffrent records.
    #[derive(Debug)]
    #[repr(u8)]
    pub enum MADTRecord {
        LocalAPIC(LocalAPIC) = 0,
        IOAPIC(IOAPIC) = 1,
        IOAPICInteruptSourceOveride(IOAPICInteruptSourceOveride) = 2,
        IOAPICNonMaskableInteruptSource(IOAPICNonMaskableInteruptSource) = 3,
        LocalAPICNonMaskableInteruptSource(LocalAPICOveride) = 4,
        LocalAPICOveride(LocalAPICOveride) = 5,
        LocalAPIC2X(LocalAPIC2X) = 6,
    }
}

/// A easy iterator over the MADT registers.
#[derive(Debug)]
#[repr(C, packed)]
pub struct MADTRecordIterator {
    pointer: *const u8,
    end: *const u8,
}

impl Iterator for MADTRecordIterator {
    type Item = &'static record::MADTRecord;
    fn next(&mut self) -> Option<Self::Item> {
        use record::{MADTRecord, MADTRecordHeader};
        if self.pointer >= self.end {
            return None;
        }
        let header = unsafe { &*self.pointer.cast::<MADTRecordHeader>() };
        let record =
            unsafe { &*core::ptr::from_ref::<MADTRecordHeader>(header).cast::<MADTRecord>() };
        self.pointer = unsafe { self.pointer.add(header.record_length as usize) };
        Some(record)
    }
}
