use core::marker::PhantomData;

use crate::memory::addresses::{Address, PhysAddr};

pub trait Alignment<T: Address> {
    const SIZE: usize;

    #[must_use]
    #[inline]
    fn align_down(addr: T) -> T {
        const { assert!(Self::SIZE.is_power_of_two()) }
        T::new(addr.to_usize() & !(Self::SIZE - 1))
    }
}

pub struct Align4Kb;

impl Alignment<PhysAddr> for Align4Kb {
    const SIZE: usize = 4096;
}

pub struct PhysFrame<S: Alignment<PhysAddr> = Align4Kb> {
    address: PhysAddr,
    size: PhantomData<S>,
}

impl<S: Alignment<PhysAddr>> PhysFrame<S> {
    #[inline]
    #[must_use]
    pub fn new(addr: PhysAddr) -> Self {
        let address = S::align_down(addr);

        Self {
            address,
            size: PhantomData,
        }
    }

    #[must_use]
    #[inline]
    pub const fn get_base(&self) -> PhysAddr {
        self.address
    }
}
