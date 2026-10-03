use crate::{
    acpi::{
        aml::executor::AmlVM,
        sdt::{SDTHeader, ShortSDTHeader},
    },
    variables::AMLVM_STACK_SIZE,
};

#[derive(Debug)]
#[repr(C, packed)]
pub struct DSDT {
    pub header: ShortSDTHeader,
}

impl DSDT {
    #[must_use]
    pub fn create_amlvm(&self) -> AmlVM {
        let start = unsafe { (core::ptr::from_ref::<Self>(self)).add(1) }.cast::<u8>();
        let length = self.header.length as usize - size_of::<SDTHeader>();
        AmlVM::new(start, length, AMLVM_STACK_SIZE)
    }
}
