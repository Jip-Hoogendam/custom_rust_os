use core::ptr::from_raw_parts;

use x86_64::{PhysAddr, VirtAddr};


use super::{SDTLoadErrors, SDTables};

/// The structure of the RSDP.
/// When loading a checksum needs to be created by adding the first 20 bytes of the RSDP
/// togeter in a u8 and seeing if it counts up to 0.
#[derive(Debug)]
#[repr(C, packed)]
pub struct RSDP {
    /// The signature of the pointer
    /// Can be used to locate the RSDP or just be ignored.
    /// Has to always be `RSD PTR ` including the space!.
    pub signature: [u8; 8],
    /// This is used to offset the result of the checksum.
    pub checksum: u8,
    /// The id of the oem.
    /// TODO: figure out what this means
    pub oemid: [u8; 6],
    /// The revision of the RSDP
    /// if this is heigher then 1 the use the XSDP instead.
    pub revision: u8,
    /// The adress of the RSDT.
    pub rsdt_address: u32,
}

/// The structure of the XSDP.
///
/// Use this structure in favor of the RSDP
///
/// When loading a checksum needs to be created by adding the length of the header bytes of the XSDP
/// togeter in a u8 and seeing if it counts up to 0.
#[derive(Debug)]
#[repr(C, packed)]
pub struct XSDP {
    /// The signature of the pointer
    /// Can be used to locate the RSDP or just be ignored.
    /// Has to always be `RSD PTR ` including the space!.
    pub signature: [u8; 8],
    /// This is used to offset the result of the checksum.
    pub checksum: u8,
    /// The id of the oem.
    /// TODO: figure out what this means
    pub oemid: [u8; 6],
    /// The revision of the RSDP
    /// if this is heigher then 1 the use the XSDP instead.
    pub revision: u8,
    /// The adress of the RSDT. Use the `xsdt_address` instead.
    pub rsdt_address: u32,

    /// The total length including in bytes of the XSDP.
    pub length: u32,
    /// The adress of the XSDT.
    pub xsdt_address: u64,
    /// The extended checksum.
    /// I guess this got included to allow backward compatibility with the RSDP.
    pub extended_checksum: u8,
    /// Don't know why you would reserve data after a struct that has a length field. but hey dont
    /// want to knock your boat.
    pub reserved: [u8; 3],
}

/// The system discriptor table pointer.
///
/// This will load eighter the extended or root system disctiptor pointer. These are required to
/// load the system discriptor table. This enum when loading will also checksum the discriptor
/// pointers.
///
/// The are 2 variants of the SDP these are loaded depending on the table that is available.
/// This is done based off the idea that if the revision is higher then 2 then the XSDT is loaded.
pub enum SDP<'a> {
    RSDP(&'a RSDP),
    XSDP(&'a XSDP),
}

impl SDP<'_> {
    /// Loads the SDP from the given pointer.
    ///
    /// # Safety
    /// This function assumes that the rsdp given pointer is actually pointing to a rsdp header.
    ///
    /// # Errors
    /// Returns a `SDTLoadErrors::InvalidChecksum` if the checksum is invalid
    pub unsafe fn load_sdp(
        rsdp_addr: u64,
        phys_mem_offset: VirtAddr,
    ) -> Result<Self, SDTLoadErrors> {
        let rsdp_phys = PhysAddr::new(rsdp_addr);
        let rsdp_virt = phys_mem_offset + rsdp_phys.as_u64();
        let rsdp = unsafe { &*rsdp_virt.as_ptr::<RSDP>() };
        if rsdp.revision > 1 {
            let xsdp = unsafe { &*rsdp_virt.as_ptr::<XSDP>() };
            let raw_data = rsdp_virt.as_ptr::<u8>();
            let mut checksum = 0u8;
            #[allow(clippy::cast_possible_wrap)]
            for i in 0..xsdp.length {
                checksum = checksum
                    .overflowing_add(unsafe { *raw_data.offset(i as isize) })
                    .0;
            }
            if checksum != 0 {
                Err(SDTLoadErrors::InvalidChecksum)
            } else {
                Ok(Self::XSDP(xsdp))
            }
        } else {
            let checksum = unsafe { &*rsdp_virt.as_ptr::<[u8; 20]>() };
            let checksum = checksum.iter().fold(0u8, |c, x| c.overflowing_add(*x).0);
            if checksum != 0 {
                Err(SDTLoadErrors::InvalidChecksum)
            } else {
                Ok(Self::RSDP(rsdp))
            }
        }
    }
}

/// The headers of the root system discriptor table headers.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct SDTHeader {
    /// The signature header a 4 char denoting what data this table holds.
    pub signature: [u8; 4],
    /// The entire length of the table in bytes including the header.
    pub length: u32,
    /// The revision. I ignore this thihi.
    pub revision: u8,
    /// The checksum. This is to add alignment for when calculating the checksum.
    pub checksum: u8,
    /// OEM id. This is prob needed for something.
    pub oem_id: [u8; 6],
    /// The OEM table id. Maybe the OEM needs to feel special.
    pub oem_table_id: [u8; 8],
    /// I think they just needed to feel special.
    /// TODO: figure out why this is needed.
    pub oem_revision: u32,
    /// TODO: figure out how this would be resolved or what it means.
    pub creator_id: u32,
    /// TODO: figure out how this would be resolved or what it means.
    pub creator_revision: u32,
}

/// The headers of the root system discriptor table headers without the discriptor.
///
/// This when coupled with smart enum packing allows for easy casting of the table discriptor into a
/// distinct enum type.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ShortSDTHeader {
    /// The entire length of the table in bytes including the header.
    pub length: u32,
    /// The revision. I ignore this thihi.
    pub revision: u8,
    /// The checksum. This is to add alignment for when calculating the checksum.
    pub checksum: u8,
    /// OEM id. This is prob needed for something.
    pub oem_id: [u8; 6],
    /// The OEM table id. Maybe the OEM needs to feel special.
    pub oem_table_id: [u8; 8],
    /// I think they just needed to feel special.
    /// TODO: figure out why this is needed.
    pub oem_revision: u32,
    /// TODO: figure out how this would be resolved or what it means.
    pub creator_id: u32,
    /// TODO: figure out how this would be resolved or what it means.
    pub creator_revision: u32,
}

impl SDTHeader {
    /// Loads the SDT header from the given adress.
    ///
    /// # Safety
    /// Expects the given address to be a pointer to the start of a real system disctiptor table
    /// header
    ///
    /// # Errors
    /// Returns a `SDTLoadErrors::InvalidChecksum` if the checksum is invalid
    pub unsafe fn load<'a>(adress: u64) -> Result<&'a Self, SDTLoadErrors> {
        let header = unsafe { &*(adress as *const u8).cast::<Self>() };
        let raw_data = (adress as *const u8).cast::<u8>();
        let mut checksum = 0u8;
        #[allow(clippy::cast_possible_wrap)]
        for i in 0..header.length {
            checksum = checksum
                .overflowing_add(unsafe { *raw_data.offset(i as isize) })
                .0;
        }
        if checksum != 0 {
            Err(SDTLoadErrors::InvalidChecksum)
        } else {
            Ok(header)
        }
    }

    /// Cast it into a specific type.
    ///
    /// # Safety
    /// This is perfectly safe to do as long as the load function was handled correctly.
    #[inline]
    #[must_use]
    pub const fn cast<T>(&self) -> &T {
        unsafe { &*core::ptr::from_ref::<Self>(self).cast::<T>() }
    }
}

/// A handly iterator to loop over the SDT records.
pub struct SDTIter<'a> {
    offset: u64,
    sdt: SDT<'a>,
    index: usize,
    table: &'a [u8],
}

/// A iterator of System discriptor tables.
impl<'a> Iterator for SDTIter<'a> {
    type Item = &'a SDTables;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.table.len() {
            return None;
        }
        match self.sdt.typ {
            SDTType::XSDT => {
                let ptr =
                    u64::from_le_bytes(self.table[self.index..self.index + 8].try_into().ok()?);
                let item = SDTables::load(ptr + self.offset).ok()?;
                self.index += size_of::<u64>();
                Some(item)
            }
            SDTType::RSDT => {
                let ptr =
                    u32::from_le_bytes(self.table[self.index..self.index + 8].try_into().ok()?);
                let item = SDTables::load(u64::from(ptr) + self.offset).ok()?;
                self.index += size_of::<u64>();
                Some(item)
            }
        }
    }
}

/// A enum denoting the type of the SDT
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug)]
enum SDTType {
    RSDT,
    XSDT,
}

/// A enum holding eighter a pointer to the RSDT or XSDT.
/// The given system disctiptor table is based off the provided  system discriptor pointer.
#[derive(Debug)]
pub struct SDT<'a> {
    typ: SDTType,
    header: &'a SDTHeader,
}

impl<'a> SDT<'a> {
    #[must_use]
    pub const fn tables_iter(self, offset: u64) -> SDTIter<'a> {
        let length = self.header.length as usize - size_of::<SDTHeader>();
        let pointer = core::ptr::from_ref::<SDTHeader>(self.header);
        let pointer = unsafe { pointer.add(1) }.cast::<u8>();
        let slice = unsafe { &*from_raw_parts::<[u8]>(pointer, length) };
        SDTIter {
            offset,
            index: 0,
            sdt: self,
            table: slice,
        }
    }

    /// Loads the SDT from the given SDP and memory offset.
    ///
    /// # Safety
    /// This should be safe to do as long as the provided SDP is correct
    ///
    /// # Errors
    /// Returns a `SDTLoadErrors::InvalidChecksum` if the checksum is invalid
    pub fn load(sdp: &SDP<'a>, phys_mem_offset: VirtAddr) -> Result<Self, SDTLoadErrors> {
        let (typ, header) = match sdp {
            SDP::RSDP(rsdp) => (SDTType::RSDT, unsafe {
                SDTHeader::load(phys_mem_offset.as_u64() + u64::from(rsdp.rsdt_address))?.cast()
            }),
            SDP::XSDP(xsdp) => (SDTType::XSDT, unsafe {
                SDTHeader::load(phys_mem_offset.as_u64() + xsdp.xsdt_address)?.cast()
            }),
        };

        Ok(Self { typ, header })
    }
}
