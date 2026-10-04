use core::{arch::asm, marker::PhantomData};

pub struct Port<T: Copy> {
    port: u16,
    data: PhantomData<T>,
}

impl<T: Copy> Port<T> {
    /// # Panics
    /// This function panics if the size of `T` is not a multiple of 2 or lower or equal to 4
    #[must_use]
    pub const fn new(port: u16) -> Self {
        const {
            assert!(size_of::<T>() <= 4 && size_of::<T>().is_power_of_two());
        }
        Self {
            port,
            data: PhantomData,
        }
    }

    #[must_use]
    pub fn read(&self) -> T {
        match size_of::<T>() {
            1 => unsafe {
                let data: u8;
                asm! {"in al, dx", out("al") data, in("dx") self.port, options(nomem, nostack, preserves_flags)}
                *((&raw const data).cast())
            },
            2 => unsafe {
                let data: u16;
                asm! {"in {D:x}, dx",  D = out(reg) data, in("dx") self.port, options(nomem, nostack, preserves_flags)}
                *((&raw const data).cast())
            },
            4 => unsafe {
                let data: u32;
                asm! {"in {D:e}, dx",  D = out(reg) data, in("dx") self.port, options(nomem, nostack, preserves_flags)}
                *((&raw const data).cast())
            },
            _ => unreachable!(),
        }
    }

    pub fn write(&mut self, data: T) {
        match size_of::<T>() {
            1 => unsafe {
                let data: u8 = *(&raw const data).cast();
                asm! {"out dx, al",   in("dx") self.port, in("al") data, options(nomem, nostack, preserves_flags)}
            },
            2 => unsafe {
                let data: u16 = *(&raw const data).cast();
                asm! {"out dx, ax", in("dx") self.port, in("ax") data, options(nomem, nostack, preserves_flags)}
            },
            4 => unsafe {
                let data: u32 = *(&raw const data).cast();
                asm! {"out dx, eax", in("dx") self.port, in("eax") data,  options(nomem, nostack, preserves_flags)}
            },
            _ => unreachable!(),
        }
    }
}
