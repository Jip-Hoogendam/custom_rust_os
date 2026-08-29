use core::{arch::asm, mem::transmute};

use crate::{
    PrivalageLevel,
    interrupts::interupt_gaurd::{InteruptController, InteruptGaurd},
    tables::{gdt::SegmentSelector, tss::InteruptStack},
};

/// A enum to interact with the Interupt gate type
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InteruptGateType {
    InteruptGate = 0xE,
    TrapGate = 0xF,
}

/// Used to interact with the flags of a `InteruptDiscriptor`.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C, packed)]
struct InteruptDiscriptorAttributes {
    bits: u8,
}

impl InteruptDiscriptorAttributes {
    const GATE_OFFSET: u8 = 0;
    const DPL_OFFSET: u8 = 5;
    const P_OFFSET: u8 = 7;

    /// Creates a new value with all flags unset
    #[inline]
    #[must_use]
    const fn new() -> Self {
        Self { bits: 0 }
    }

    /// Sets bits of the flag at a offset
    #[inline]
    #[must_use]
    const fn set_bits(self, data: u8, offset: u8) -> Self {
        Self {
            bits: self.bits | data << offset,
        }
    }

    /// Unsets the bits of the flag as ta offset
    #[inline]
    #[must_use]
    const fn unset_bits(self, data: u8, offset: u8) -> Self {
        Self {
            bits: self.bits & !(data << offset),
        }
    }

    /// Sets the gate type of the Interupt
    #[inline]
    #[must_use]
    pub const fn set_gate_type(self, kind: InteruptGateType) -> Self {
        self.set_bits(kind as u8, Self::GATE_OFFSET)
    }

    /// Sets what CPU privalge level is allowed to acess the prosess.
    #[inline]
    #[must_use]
    pub const fn set_privalage_level(self, level: PrivalageLevel) -> Self {
        self.set_bits(level as u8, Self::DPL_OFFSET)
    }

    /// Sets the present bit to enable this interupt
    #[inline]
    #[must_use]
    pub const fn enable(self) -> Self {
        self.set_bits(1, Self::P_OFFSET)
    }

    /// Unsets the present bit to disable this interupt
    #[inline]
    #[must_use]
    pub const fn disable(self) -> Self {
        self.unset_bits(0, Self::P_OFFSET)
    }

    pub const fn is_enabled(self) -> bool {
        self.bits & 1 << Self::P_OFFSET != 0
    }
}

/// A single interupt discriptor.
///
/// The interupt discriptor describes how a interupt is handled. These discriptors are placed into
/// the `InteruptDiscriptorTable`. There are 2 types of interupt the Trap gate  and the interupt
/// gate. The diffrence is that in a iterupt gate interupts are disabled on entry and exit of iret
/// and in a trap gate this does not happen.
#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct InteruptDiscriptor {
    /// The offset points to the virtual pointer of the stack used.
    offset_1: u16,
    /// The selector should point to a valid code segment the interupt will return into.
    selector: SegmentSelector,
    /// The selector allows for pointing to a stack in the tss to be used during the interupt  
    ist: InteruptStack,
    /// The following attributes can be set
    /// |bits|name|discription|
    /// |0-3|Gate Type| Set eighter to a trap(0xF) or interupt gate|
    /// |4|0|Must be set to 0|
    /// |5-6|DPL|the privalage level that is allowed to acess this interupt|
    /// |7|P|Present bit. Must be set to 1 to enable this interupt|
    atters: InteruptDiscriptorAttributes,
    offset_2: u16,
    offset_3: u32,
    reserved: u32,
}

impl InteruptDiscriptor {
    /// Creates a new discriptor with pointing to nothing
    #[inline]
    #[must_use]
    pub const fn null() -> Self {
        Self {
            offset_1: 0,
            selector: unsafe { transmute::<u16, SegmentSelector>(0) },
            ist: unsafe { transmute::<u8, InteruptStack>(0) },
            atters: InteruptDiscriptorAttributes { bits: 0 },
            offset_2: 0,
            offset_3: 0,
            reserved: 0,
        }
    }

    /// Sets the segment selector
    #[inline]
    #[must_use]
    const fn set_selctor(mut self, selector: SegmentSelector) -> Self {
        self.selector = selector;
        self
    }

    /// Sets the offset fields of the descriptor
    #[inline]
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    const fn set_offset(mut self, offset: u64) -> Self {
        self.offset_1 = offset as u16;
        self.offset_2 = (offset >> 16) as u16;
        self.offset_3 = (offset >> 32) as u32;
        self
    }

    /// Sets the descriptor attributes
    #[inline]
    #[must_use]
    const fn set_atters(mut self, atters: InteruptDiscriptorAttributes) -> Self {
        self.atters = atters;
        self
    }

    /// creates a new descriptor from the offset.
    /// The selector will be set to the value from the cs register.
    #[must_use]
    pub const fn new_interupt(offset: u64, selector: SegmentSelector) -> Self {
        Self::null()
            .set_offset(offset)
            .set_selctor(selector)
            .set_atters(
                InteruptDiscriptorAttributes::new()
                    .enable()
                    .set_gate_type(InteruptGateType::InteruptGate)
                    .set_privalage_level(PrivalageLevel::Ring0),
            )
    }

    #[must_use]
    pub const fn new_trap(offset: u64, selector: SegmentSelector) -> Self {
        Self::null()
            .set_offset(offset)
            .set_selctor(selector)
            .set_atters(
                InteruptDiscriptorAttributes::new()
                    .enable()
                    .set_gate_type(InteruptGateType::TrapGate)
                    .set_privalage_level(PrivalageLevel::Ring0),
            )
    }

    #[must_use]
    pub const fn new_user_space_interupt(offset: u64, selector: SegmentSelector) -> Self {
        Self::null()
            .set_offset(offset)
            .set_selctor(selector)
            .set_atters(
                InteruptDiscriptorAttributes::new()
                    .enable()
                    .set_gate_type(InteruptGateType::InteruptGate)
                    .set_privalage_level(PrivalageLevel::Ring3),
            )
    }

    #[inline]
    #[must_use]
    pub const fn set_stack(mut self, stack: InteruptStack) -> Self {
        self.ist = stack;
        self
    }

    /// Disables the interupt
    pub const fn disable(&mut self) {
        self.atters = self.atters.disable();
    }

    /// Enables the interupt
    pub const fn enable(&mut self) {
        self.atters = self.atters.enable();
    }

    /// Sees if the interupt is enabled
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.atters.is_enabled()
    }
}

#[repr(u8)]
pub enum InteruptDiscriptorIndex {
    /// Triggerd by Faults with DIV and IDIV instructions.
    DevideError = 0x00,
    DebugExeption = 0x01,
    /// A `NonMaskable` external interupt
    NMIInterupt = 0x02,
    /// Triggerd by the INT3 instruction
    Breakpoint = 0x03,
    /// Triggerd by the the INTO instruction
    Overflow = 0x04,
    /// Triggerd by the the BOUND instruction
    BOUNDRangeExeeded = 0x05,
    /// Triggerd by calling a Undefined of reserved opcode.
    InvalidOpcode = 0x06,
    /// Triggerd by Floating point or WAIT/FWAIT instructions.
    DeviceNotAvailable = 0x07,
    /// Triggerd when there a interupt gets called that cannot be handled.
    /// Pushes a error code (always 0).
    DoubleFault = 0x08,
    /// Triggerd on floating point instructions
    CoprocessorSegmentOverrun = 0x09,
    /// Triggerd on accessing the TSS but it it invalid,
    /// Pushes a error code
    InvalidTSS = 0x0A,
    /// Triggerd on loading or accessing a invlid segment.
    /// Pushes a error code
    SegmentNotPresent = 0x0B,
    /// Triggerd on stack operations or loads.
    /// Pushes a error code
    StackSegmentFault = 0x0C,
    /// Triggerd on any memory refrence and protetection checks.
    /// Pushes a error code
    GeneralProtection = 0x0D,
    /// Triggerd on any invalid memory refrence.
    /// Pushes a error code
    PageFault = 0x0E,
    /// Triggerd on any faults wit the x87 FPU floating-point or WAIT/FWAIT instructions.
    X87FPUFLoatingPointError = 0x10,
    /// Triggerd on refrenceing any unaligned data.
    /// Pushes a error code (always 0)
    AlignmentCheck = 0x11,
    /// Triggerd on any error code by the machine.
    MachineCheck = 0x12,
    /// Triggerd on any invalid SSE/SSE2/SSE3 floating point instructions.
    SIMDFloatingPointException = 0x13,
    /// Triggerd by any virtulaization exceptions.
    VirtulaizationException = 0x14,
    /// Triggerd on any RET, IRET, RSTORSSP and SETSSPSY when CET interect tracking is enbaled.
    ControlProtectionException = 0x15,

    /// Any beyond this point are purely assinged by me. All are past the 0x32 to respect the IDT spec.

    /// This interupt is called when the APIC timer requests a task switch.
    TaskSwitch = 0x50,
}

/// The structure that houses the interupt discriptor table.
#[derive(Debug)]
#[repr(C, packed)]
pub struct InteruptDiscriptorTable {
    discriptors: [InteruptDiscriptor; 256],
}

impl Default for InteruptDiscriptorTable {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl InteruptDiscriptorTable {
    /// Creates a new discriptor table with no entries
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            discriptors: [InteruptDiscriptor::null(); 256],
        }
    }

    /// Sets a discriptor at a specific index
    pub const fn set_discriptor<T>(
        &mut self,
        index: InteruptDiscriptorIndex,
        discriptor: InteruptDiscriptor,
        _interupt_gaurd: &InteruptGaurd<T>,
    ) where
        T: InteruptController,
    {
        self.discriptors[index as usize] = discriptor;
    }

    /// Loads the idt into the idtr
    pub fn load<T>(&self, _interupt_gaurd: &InteruptGaurd<T>)
    where
        T: InteruptController,
    {
        #[derive(Debug)]
        #[repr(C, packed(2))]
        struct Idtr {
            length: u16,
            pointer: u64,
        }
        #[allow(clippy::cast_possible_truncation)]
        let length = size_of::<Self>() as u16 - 1;
        let pointer = core::ptr::from_ref::<Self>(self) as u64;
        let idtr = &Idtr { length, pointer };
        unsafe {
            asm!("lidt [{idtr}]", idtr =  in(reg) idtr, options(readonly, nostack, preserves_flags));
        }
    }
}
