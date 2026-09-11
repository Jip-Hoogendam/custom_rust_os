use core::{
    marker::PhantomData,
    ops::{Add, AddAssign},
};

use alloc::slice;

use crate::memory::addresses::{Address, PhysAddr, PhysMemOffset};

pub trait FrameSize {
    const SIZE: usize;

    #[must_use]
    #[inline]
    fn align_down(addr: PhysAddr) -> PhysAddr {
        const { assert!(Self::SIZE.is_power_of_two()) }
        PhysAddr::new(addr.to_usize() & !(Self::SIZE - 1))
    }
}

pub struct Frame4Kb;

impl FrameSize for Frame4Kb {
    const SIZE: usize = 4096;
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct PhysFrame<S: FrameSize = Frame4Kb> {
    address: PhysAddr,
    size: PhantomData<S>,
}

impl<S: FrameSize> AddAssign<usize> for PhysFrame<S> {
    #[inline]
    fn add_assign(&mut self, rhs: usize) {
        self.address += rhs * Self::SIZE;
    }
}

impl<S: FrameSize> Add<usize> for PhysFrame<S> {
    type Output = Self;

    #[inline]
    fn add(self, rhs: usize) -> Self::Output {
        Self {
            address: self.address + rhs * Self::SIZE,
            size: PhantomData,
        }
    }
}

impl<S: FrameSize> Eq for PhysFrame<S> {}
impl<S: FrameSize> PartialEq for PhysFrame<S> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address
    }
}

impl<S: FrameSize> PhysFrame<S> {
    pub const SIZE: usize = S::SIZE;

    #[inline]
    #[must_use]
    pub fn new(addr: PhysAddr) -> Self {
        let address = S::align_down(addr);

        Self {
            address,
            size: PhantomData,
        }
    }

    pub fn zero(&mut self, offset: PhysMemOffset) {
        unsafe {
            let slice = slice::from_raw_parts_mut(
                (self.address + offset).to_mut_ptr::<usize>(),
                S::SIZE / size_of::<usize>(),
            );
            for block in slice {
                *block = 0;
            }
        }
    }

    #[must_use]
    #[inline]
    pub const fn get_base(&self) -> PhysAddr {
        self.address
    }

    /// Returns the index of the frame from the beginning of the memory
    #[inline]
    #[must_use]
    pub fn index(&self) -> usize {
        self.get_base().to_usize() / S::SIZE
    }
}
