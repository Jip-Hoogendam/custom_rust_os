use crate::tables::idt::InteruptDiscriptorIndex;

#[repr(u8)]
pub enum IRQ {
    Pit = 0,
    Keyboard = 1,
    Cascade = 2, // According to osdev wiki this is never raised
    COM2 = 3,
    COM1 = 4,
    LPT2 = 5,
    FloppyDisk = 6,
    LPT1 = 7,
    CMOSRtc = 8,
    Perifirals1 = 9,
    Perifirals2 = 10,
    Perifirals3 = 11,
    PS2Mouse = 12,
    FPUCoprocessor = 13,
    PrimaryATA = 14,
    SecondaryATA = 15,
}

/// A overachring trait for a programmable interupt controller.
pub trait PIC {
    fn irq_subscribe(irq: IRQ, vector: InteruptDiscriptorIndex);
}
