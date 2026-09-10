use bootloader_api::{BootInfo, info::Optional};
use core::ops::{Add, AddAssign, Sub};

pub trait Address {
    #[must_use]
    fn to_usize(&self) -> usize;

    #[must_use]
    fn new(addr: usize) -> Self;
}

/// A simple tool to interact with the physical mmeory offset
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct PhysMemOffset(VirtAddr);

impl PhysMemOffset {
    #[must_use]
    #[inline]
    pub const fn get(boot_info: &BootInfo) -> Self {
        if let Optional::Some(addr) = boot_info.physical_memory_offset {
            #[allow(clippy::cast_possible_truncation)]
            Self(VirtAddr::new(addr as usize))
        } else {
            //If there is no physical memory offset then just return 0
            Self(VirtAddr(0))
        }
    }

    /// To convert from virt addr
    #[inline]
    #[must_use]
    pub const fn from_virt(addr: VirtAddr) -> Self {
        Self(addr)
    }

    #[inline]
    #[must_use]
    pub const fn inner(&self) -> VirtAddr {
        self.0
    }

    //Converts this type to a x86_64 lib variant while i am transitioning away from this crate
    #[inline]
    #[must_use]
    pub const fn to_x86_lib(&self) -> x86_64::VirtAddr {
        self.0.to_x86_lib()
    }
}

impl Add<PhysAddr> for PhysMemOffset {
    type Output = VirtAddr;
    #[inline]
    fn add(self, rhs: PhysAddr) -> Self::Output {
        VirtAddr::new(self.0.0 + rhs.0)
    }
}

/// A simple struct to interact with physical addresses
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysAddr(usize);

impl Address for PhysAddr {
    #[inline]
    fn to_usize(&self) -> usize {
        self.0
    }

    #[inline]
    fn new(addr: usize) -> Self {
        Self(addr)
    }
}

impl PhysAddr {
    #[must_use]
    #[inline]
    pub const fn to_virt(&self, offset: PhysMemOffset) -> VirtAddr {
        VirtAddr::new(self.0 + offset.0.0)
    }

    //Converts this type to a x86_64 lib variant while i am transitioning away from this crate
    #[inline]
    #[must_use]
    pub const fn to_x86_lib(&self) -> x86_64::PhysAddr {
        x86_64::PhysAddr::new(self.0 as u64)
    }
}

impl From<u32> for PhysAddr {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value as usize)
    }
}

impl From<u64> for PhysAddr {
    #[inline]
    fn from(value: u64) -> Self {
        #[allow(clippy::cast_possible_truncation)]
        Self(value as usize)
    }
}

impl From<PhysAddr> for usize {
    fn from(val: PhysAddr) -> Self {
        val.0
    }
}

impl Add<PhysMemOffset> for PhysAddr {
    type Output = VirtAddr;
    #[inline]
    fn add(self, rhs: PhysMemOffset) -> Self::Output {
        rhs + self
    }
}

impl AddAssign<usize> for PhysAddr {
    #[inline]
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs;
    }
}

impl Add<usize> for PhysAddr {
    type Output = Self;
    #[inline]
    fn add(self, rhs: usize) -> Self::Output {
        Self::new(self.to_usize() + rhs)
    }
}

impl Sub<usize> for PhysAddr {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: usize) -> Self::Output {
        Self::new(self.to_usize() - rhs)
    }
}

impl Add<u64> for PhysAddr {
    type Output = Self;
    #[inline]
    fn add(self, rhs: u64) -> Self::Output {
        #[allow(clippy::cast_possible_truncation)]
        Self::new(self.to_usize() + rhs as usize)
    }
}

impl Sub<u64> for PhysAddr {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: u64) -> Self::Output {
        #[allow(clippy::cast_possible_truncation)]
        Self::new(self.to_usize() - rhs as usize)
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct VirtAddr(usize);

impl Address for VirtAddr {
    fn to_usize(&self) -> usize {
        self.0
    }

    #[inline]
    fn new(addr: usize) -> Self {
        Self(addr)
    }
}

impl VirtAddr {
    #[must_use]
    #[inline]
    pub const fn new(addr: usize) -> Self {
        Self(addr)
    }

    #[must_use]
    #[inline]
    pub fn to_phys(&self, offset: PhysMemOffset) -> PhysAddr {
        PhysAddr::new(self.0 - offset.0.0)
    }

    //Converts this type to a x86_64 lib variant while i am transitioning away from this crate
    #[inline]
    #[must_use]
    pub const fn to_x86_lib(&self) -> x86_64::VirtAddr {
        x86_64::VirtAddr::new(self.0 as u64)
    }

    #[inline]
    #[must_use]
    pub const fn to_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }

    #[inline]
    #[must_use]
    pub const fn to_mut_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }

    /// # Safety
    /// This function derefrences a raw pointer. This comes with the same safety concerns that the
    /// normal derefecing comes with. Added with the fact that may crash if the given pointer is not
    /// mapped
    #[inline]
    #[must_use]
    pub const unsafe fn to_mut_ref<T>(&self) -> &'static mut T {
        unsafe { &mut *self.to_mut_ptr() }
    }

    /// # Safety
    /// This function derefrences a raw pointer. This comes with the same safety concerns that the
    /// normal derefecing comes with. Added with the fact that may crash if the given pointer is not
    /// mapped
    #[inline]
    #[must_use]
    pub const unsafe fn to_ref<T>(&self) -> &'static T {
        unsafe { &*self.to_ptr() }
    }
}

impl From<u32> for VirtAddr {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value as usize)
    }
}

impl From<u64> for VirtAddr {
    #[inline]
    fn from(value: u64) -> Self {
        #[allow(clippy::cast_possible_truncation)]
        Self(value as usize)
    }
}

impl From<VirtAddr> for usize {
    fn from(val: VirtAddr) -> Self {
        val.0
    }
}

impl Sub<PhysMemOffset> for VirtAddr {
    type Output = PhysAddr;
    fn sub(self, rhs: PhysMemOffset) -> Self::Output {
        PhysAddr::new(self.0 - rhs.0.0)
    }
}
