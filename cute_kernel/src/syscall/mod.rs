use core::arch::asm;

use crate::{
    interrupts::ExceptionStackFrame, serial_println, tables::idt::InteruptDiscriptorIndex,
};

pub extern "C" fn syscall_handeler(
    mut syscall: SysCalls,
    mut var_1: usize,
    mut var_2: usize,
    mut var_3: usize,
) {
    serial_println!(
        "Foo! : {:?}  var_1 : {} var_2 : {} var_3 : {}",
        syscall,
        var_1,
        var_2,
        var_3
    );
}

#[derive(Debug)]
#[repr(usize)]
pub enum SysCalls {
    ///These syscalls are based off linux
    Read = 0,
    Write = 1,
    Open = 2,
    Close = 3,

    ///Beyond linux syscalls for inter kernal use only
    MtxLock = 500,
}

#[inline]
pub fn syscall(
    syscall: SysCalls,
    mut var_1: usize,
    mut var_2: usize,
    mut var_3: usize,
) -> (usize, usize, usize, usize) {
    let mut syscall = syscall as usize;
    unsafe {
        asm! {"int {x}", x = const(InteruptDiscriptorIndex::SysCall as u8), inout("edi") syscall, inout("esi") var_1, inout("edx") var_2, inout("ecx") var_3}
    }
    (syscall as usize, var_1, var_2, var_3)
}
