use crate::{
    acpi::{
        aml::{
            executor::{AmlVM, ByteStream},
            namenode::AmlData,
        },
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
    pub fn load_byte_stream(&self) -> ByteStream {
        let start = unsafe { (core::ptr::from_ref::<Self>(self)).add(1) }.cast::<u8>();
        let length = self.header.length as usize - size_of::<SDTHeader>();
        ByteStream::new(start, length)
    }

    #[must_use]
    pub fn create_amlvm(&self) -> AmlVM {
        AmlVM::new(self.load_byte_stream(), AMLVM_STACK_SIZE)
    }
}
