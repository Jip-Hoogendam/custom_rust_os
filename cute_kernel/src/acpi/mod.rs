use x86_64::VirtAddr;

pub mod apic;
pub mod madt;
pub mod sdt;
use madt::MADT;
use sdt::{SDP, SDT, SDTHeader};

use crate::acpi::sdt::ShortSDTHeader;

#[derive(Debug)]
#[non_exhaustive]
#[repr(u32, C)]
pub enum SDTables {
    /// # Multiple APIC Discriptor Table.
    /// This table contains all the information of each (I/O) APIC.
    ///
    /// This signature is adjusted to `APIC`.
    MADT(MADT) = u32::from_le_bytes(*b"APIC"),
    /// # Boot grapics Resource Table.
    ///
    /// Holds data about the displayed image during boot.
    BGRT(ShortSDTHeader) = u32::from_le_bytes(*b"BGRT"),
    DTST(ShortSDTHeader) = u32::from_le_bytes(*b"DTST"),
    FADT(ShortSDTHeader) = u32::from_le_bytes(*b"FACP"),
    /// #Root System Discriptor table.
    ///
    /// This is here for completeness.
    /// Holds refrences to all other discriptor tables.
    RSDT(ShortSDTHeader) = u32::from_le_bytes(*b"RSDT"),
    SRAT(ShortSDTHeader) = u32::from_le_bytes(*b"SRAT"),
    SSDT(ShortSDTHeader) = u32::from_le_bytes(*b"SSDT"),
    /// # Extended Root System Discriptor Table.
    ///
    /// This is here for completeness.
    /// Holds refrences to all other discriptor tables.
    XSDT(ShortSDTHeader) = u32::from_le_bytes(*b"XSDT"),

    /// Not yet defined by me
    UNKOWN(ShortSDTHeader),
}

impl<'a> SDTables {
    #[inline]
    pub fn load(adress: u64) -> Result<&'a Self, SDTLoadErrors> {
        Ok(unsafe { SDTHeader::load(adress)?.cast::<Self>() })
    }
}

// Errors reeturd when loading the sdt
#[derive(Debug)]
pub enum SDTLoadErrors {
    InvalidChecksum,
}

/// Loads the acpi tables.
///
/// # Safety
/// This requires that the provided rsdp pointer is pointing at the adress of the rsdp
pub unsafe fn load_sd_table<'a>(
    rsdp_ptr: u64,
    phys_mem_offset: VirtAddr,
) -> Result<SDT<'a>, SDTLoadErrors> {
    let sdp = unsafe { SDP::load_sdp(rsdp_ptr, phys_mem_offset)? };
    SDT::load(sdp, phys_mem_offset)
}
