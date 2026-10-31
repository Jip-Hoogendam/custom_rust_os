pub mod aml;
pub mod apic;
pub mod dtst;
pub mod ioapic;
pub mod madt;
pub mod sdt;

use bootloader_api::BootInfo;
use madt::MADT;
use sdt::{SDP, SDT, SDTHeader};

use crate::{
    acpi::{dtst::DSDT, sdt::ShortSDTHeader},
    memory::addresses::{Address, PhysAddr, PhysMemOffset, VirtAddr},
};

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
    DSDT(DSDT) = u32::from_le_bytes(*b"DSDT"),
    FADT(ShortSDTHeader) = u32::from_le_bytes(*b"FACP"),
    /// #Root System Discriptor table.
    ///
    /// This is here for completeness.
    /// Holds refrences to all other discriptor tables.
    RSDT(ShortSDTHeader) = u32::from_le_bytes(*b"RSDT"),
    SRAT(ShortSDTHeader) = u32::from_le_bytes(*b"SRAT"),
    SSDT(DSDT) = u32::from_le_bytes(*b"SSDT"),
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
    /// Returns the loaded SDT table.
    ///
    /// # Errors
    /// Returns a `SDTLoadErrors::InvalidChecksum` if the checksum is invalid
    pub fn load(adress: VirtAddr) -> Result<&'a Self, SDTLoadErrors> {
        Ok(unsafe { SDTHeader::load(adress)?.cast::<Self>() })
    }
}

// Errors reeturd when loading the sdt
#[derive(Debug)]
pub enum SDTLoadErrors {
    InvalidChecksum,
    NoRSDP,
}

/// Loads the acpi tables.
///
/// # Safety
/// This requires that the provided rsdp pointer is pointing at the adress of the rsdp
///
/// # Errors
/// Returns a `SDTLoadErrors::InvalidChecksum` if the checksum is invalid
pub unsafe fn load_sd_table<'a>(
    boot_info: &BootInfo,
    phys_mem_offset: PhysMemOffset,
) -> Result<SDT<'a>, SDTLoadErrors> {
    if let bootloader_api::info::Optional::Some(rsdp_ptr) = boot_info.rsdp_addr {
        #[allow(clippy::cast_possible_truncation)]
        let rsdp_ptr = PhysAddr::new(rsdp_ptr as usize);
        let sdp = unsafe { SDP::load_sdp(rsdp_ptr, phys_mem_offset)? };
        SDT::load(&sdp, phys_mem_offset)
    } else {
        Err(SDTLoadErrors::NoRSDP)
    }
}
