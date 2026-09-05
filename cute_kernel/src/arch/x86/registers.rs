use core::arch::asm;

use crate::memory::{
    addresses::{Address, PhysAddr},
    frames::PhysFrame,
};

pub trait Register {
    fn read() -> Self;
}

pub struct Cr3(usize);

impl Register for Cr3 {
    fn read() -> Self {
        let data;
        unsafe {
            asm!("mov {}, cr3", out(reg) data, options(nomem, nostack, preserves_flags));
        }
        Self(data)
    }
}

impl Cr3 {
    #[inline]
    #[must_use]
    pub fn l4_table_frame(&self) -> PhysFrame {
        let addr = PhysAddr::new(self.0 & !0xFFF);
        PhysFrame::new(addr)
    }
}
