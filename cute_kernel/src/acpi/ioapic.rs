#[repr(C, packed(4))]
pub struct IOApic {
    id: u32,
    version: u32,
    arbitration: u32,
    entries: [u64; 24],
}
