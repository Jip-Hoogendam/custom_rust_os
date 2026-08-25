use crate::{PrivalageLevel, tables::gdt::SegmentSelector};

/// A enum to interact with the Interupt gate type
#[repr(u8)]
pub enum InteruptGateType {
    InteruptGate = 0xE,
    TrapGate = 0xF,
}

/// Used to interact with the flags of a InteruptDiscriptor.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C, packed)]
struct Flags {
    bits: u16,
}

impl Flags {
    const IST_OFFSET: u8 = 0;
    const GATE_OFFSET: u8 = 7;
    const DPL_OFFSET: u8 = 13;
    const P_OFFSET: u8 = 15;

    /// Sets bits of the flag at a offset
    #[inline]
    const fn set_bits(&self, data: u16, offset: u8) -> Self {
        Self {
            bits: self.bits | data << offset,
        }
    }

    /// Unsets the bits of the flag as ta offset
    #[inline]
    const fn unset_bits(&self, data: u16, offset: u8) -> Self {
        Self {
            bits: self.bits & !(data << offset),
        }
    }

    /// Sets the gate type of the Interupt
    #[inline]
    pub const fn set_gate_type(&self, kind: InteruptGateType) -> Self {
        self.set_bits(kind as u16, Self::IST_OFFSET)
    }

    /// Sets what CPU privalge level is allowed to acess the prosess.
    #[inline]
    pub const fn set_privalage_level(&self, level: PrivalageLevel) -> Self {
        self.set_bits(level as u16, Self::DPL_OFFSET)
    }

    /// Sets the present bit to enable this interupt
    #[inline]
    pub const fn enable(&self) -> Self {
        self.set_bits(1, Self::P_OFFSET)
    }

    /// Unsets the present bit to disable this interupt
    pub const fn disable(&self) -> Self {
        self.unset_bits(0, Self::P_OFFSET)
    }
}

/// A single interupt discriptor.
/// The interupt discriptor describes how a interupt is handled. These discriptors are placed into
/// the InteruptDiscriptorTable. There are 2 types of interupt the Trap gate  and the interupt
/// gate. The diffrence is that in a iterupt gate interupts are disabled on entry and exit of iret
/// and in a trap gate this does not happen.
#[repr(C, packed)]
pub struct InteruptDiscriptor {
    /// The offset points to the virtual pointer of the stack used.
    offset_1: u16,
    /// The selector should point to a valid code segment the interupt will return into.
    selector: SegmentSelector,
    /// The selector allows for pointing to a stack in the tss to be used during the interupt  
    ist: u8,
    /// The following attributes can be set
    /// |bits|name|discription|
    /// |0-3|Gate Type| Set eighter to a trap(0xF) or interupt gate|
    /// |4|0|Must be set to 0|
    /// |5-6|DPL|the privalage level that the interupt will return to|
    /// |7|P|Present bit. Must be set to 1 to enable this interupt|
    atters: u8,
    offset_2: u16,
    offset_3: u32,
    reserved: u32,
}

pub struct InteruptDiscriptorTable {}
