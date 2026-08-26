use crate::{
    PrivalageLevel,
    tables::{
        gdt::{
            GlobalDescriptorTable, GlobalDescriptorTableBuilder, SegmentDiscriptor,
            SegmentSelector, SystemSegmentDiscriptor,
        },
        idt::InteruptDiscriptorTable,
        tss::TaskStateSegment,
    },
};

// A structure to save all the discriptor tables in. Once these get loaded they must stay in memory
// else it may cause errors.
//
// Has build in functions to acess the idt, gdt or tss.
pub struct DiscriptorTables {
    gdt: GlobalDescriptorTableBuilder<{ crate::variables::GDT_TABLE_SIZE }>,
    idt: InteruptDiscriptorTable,
    tss: TaskStateSegment,
}

impl DiscriptorTables {
    #[must_use]
    pub const fn init() -> Self {
        let tss = TaskStateSegment::new();
        let idt = InteruptDiscriptorTable::new();
        let gdt = GlobalDescriptorTableBuilder::new();
        DiscriptorTables { gdt, tss, idt }
    }

    // Gets the idt as a mutable pointer
    #[inline]
    #[must_use]
    pub fn get_mut_idt(&mut self) -> &mut InteruptDiscriptorTable {
        &mut self.idt
    }

    /// Gets the tss as a mutable pointer
    #[inline]
    #[must_use]
    pub fn get_mut_tss(&mut self) -> &mut TaskStateSegment {
        &mut self.tss
    }

    /// Gets the gdt as a mutable pointer
    #[inline]
    #[must_use]
    pub fn get_mut_gdt(
        &mut self,
    ) -> &mut GlobalDescriptorTableBuilder<{ crate::variables::GDT_TABLE_SIZE }> {
        &mut self.gdt
    }

    pub fn load(&mut self) {
        self.gdt.load();
    }
}

pub mod gdt;
/// A module to interact with the InteruptDiscriptorTable.
/// This only suppports 64 bit long mode interupt discriptors.
pub mod idt;
pub mod tss;
