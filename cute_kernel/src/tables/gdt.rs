/// A module to interact with the GDT <https://wiki.osdev.org/Global_Descriptor_Table>.
use crate::{PrivalageLevel, println, tables::tss::TaskStateSegment};
use core::{arch::asm, fmt::Display, mem::transmute};

/// Used by the Global Descriptor table to determin the permissions of the diffrent segments. These
/// hold induvidual discriptors of each segments and denote the permissions of these files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C, packed)]
pub struct SegmentDiscriptor {
    /// All limit fields add up to a 20 bit value that indicates the length of the segment. But in
    /// 64 bit mode i can mostly ignore this. Especially since i want to use pageing to protect my
    /// memory access. In 32-bit mode this limit is in bytes. And in 64 bit mode this is in 4KiB
    /// pages.
    limit_1: u16,
    /// All base fields add up to a 32 bit vallue inidcating te start adress of the segment.
    base_1: u16,
    base_2: u8,
    /// This field is to control access to the segment. This contains 7 disctinct fields that can be
    /// set or unset. The fields are as follows
    /// |bit(s)|short|discription|
    /// |-|-|-|
    /// |0|A|This indicates the acess bit. This is set to 1 if the cpu if it tries to accsess this segment. This should only be set to 1 unnless needed|
    /// |1|RW|This indicates if the segmet is readable for a code segment. And writable for code segments|
    /// |2|DC|Read the documentation. i dont really get how this works TODO: figure out how this works|
    /// |3|E|If this bit is set to 0 this segment is a data segment. If it is 1 then it is a ata segment|
    /// |4|S|If this bit is 0 this discriptor defines a system segment and if it is 1 then data/code segment|
    /// |5-6|DPL|What privalage level is allowed to access this segment|
    /// |7|P|This must be set to enable this segment|
    access_byte: u8,
    /// The first 4 bits of this field are used for the linit. And the last four bits are used for
    /// the followig purpose
    /// |bit(s)|short|discription|
    /// |-|-|-|
    /// |0|reseverd| - |
    /// |1|L|If this is set to one then this segment a 64 bit one. When set `DB` must be clear|
    /// |2|DB|If clear this segment is 16 bit. If set this segment is 32 bit|
    /// |3|G|If clear the `length` is in 1 byte blocks. id set the length is in 4KiB pages|
    limit_flags: u8,
    base_3: u8,
}

/// This is for the system segment discriptors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C, packed)]
pub struct SystemSegmentDiscriptor {
    // reuse the segment discriptor
    top: SegmentDiscriptor,
    base_4: u32,
    reserved: u32,
}

impl SystemSegmentDiscriptor {
    const LDT_TYPE: u8 = 0x2;
    const TSS_TYPE: u8 = 0x9;

    /// Creates a new tss segment.
    #[inline]
    #[must_use]
    pub fn new_tss(tss: &TaskStateSegment) -> Self {
        let mut segment = SegmentDiscriptor::null();
        let limit = size_of::<TaskStateSegment>() - 1;

        let base = core::ptr::from_ref::<TaskStateSegment>(tss) as u64;

        // sets tss defaults
        #[allow(clippy::cast_possible_truncation)]
        segment
            .set_base(base as u32) // Sets the base of the discriptor
            .set_limit(limit) // Sets the limit of the discriptor
            .set_present(true) // Would only ever create segments that i will use
            .set_system_segment(true) //We need this to be a system segment.
            .set_segment_type(Self::TSS_TYPE);

        Self {
            top: segment,
            base_4: (base >> 32) as u32,
            reserved: 0,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_split(self) -> (SegmentDiscriptor, SegmentDiscriptor) {
        (self.top, unsafe {
            transmute::<u64, SegmentDiscriptor>(self.base_4 as u64)
        })
    }
}

impl SegmentDiscriptor {
    #[must_use]
    pub const fn null() -> Self {
        Self {
            limit_1: 0,
            base_1: 0,
            base_2: 0,
            access_byte: 0,
            limit_flags: 0,
            base_3: 0,
        }
    }

    const LIMIT_MASK: u8 = 0b0000_1111;

    #[allow(clippy::cast_possible_truncation)]
    pub const fn set_base(&mut self, base: u32) -> &mut Self {
        self.base_1 = base as u16;
        self.base_2 = (base >> 16) as u8;
        self.base_3 = (base >> 24) as u8;
        self
    }

    #[allow(clippy::cast_possible_truncation)]
    pub const fn set_limit(&mut self, mut limit: usize) -> &mut Self {
        // corrects the limit
        if limit == usize::MAX {
            self.set_granularity(true);
        }
        if limit >= 2usize.pow(20) {
            self.set_granularity(true);
            limit /= 4096;
        }
        // sets the limit
        self.limit_1 = limit as u16;
        self.limit_flags |= (limit >> 16) as u8 & Self::LIMIT_MASK;

        self
    }

    ///Creates a sector based off a base and limit
    ///The limit must be in bytes.
    #[inline]
    #[must_use]
    pub const fn new_segment(base: u32, limit: usize) -> Self {
        let mut segment = Self::null();

        // sets basic defaults
        segment
            .set_base(base) // Sets the base of the discriptor
            .set_limit(limit) // Sets the limit of the discriptor
            .set_longmode(true) // sets the segment to 64 bit mode. I wount support anything else.
            .set_present(true) // Would only ever create segments that i will use
            .set_read_write(true) // I will always want to read and write to both sectors
            .set_system_segment(false); //We need this to be a generic segment.

        segment
    }

    /// Sets the segment type
    /// When unset this segment is a data or code segment.
    /// When set this segment is a system segment.
    /// This must only be used on system segments.
    #[inline]
    pub const fn set_segment_type(&mut self, value: u8) -> &mut Self {
        const TYPE_INDEX: u8 = 0;
        const TYPE_MASK: u8 = 0x0000_0000;
        self.access_byte = (self.access_byte & !TYPE_MASK) | value << TYPE_INDEX;
        self
    }

    const S_MASK: u8 = 0b0001_0000;
    /// Sets or unsets the executable bit
    /// When unset this segment is a data or code segment.
    /// When set this segment is a system segment.
    #[inline]
    pub const fn set_system_segment(&mut self, value: bool) -> &mut Self {
        const S_INDEX: u8 = 4;
        self.access_byte = (self.access_byte & !Self::S_MASK) | (!value as u8) << S_INDEX;
        self
    }

    #[inline]
    #[must_use]
    pub const fn is_system_segment(&self) -> bool {
        !(self.access_byte & Self::S_MASK != 0)
    }

    #[inline]
    #[must_use]
    pub const fn is_tss(&self) -> bool {
        self.access_byte & 0x09 != 0 && self.is_system_segment()
    }

    /// Sets or unsets the executable bit
    /// **If this segment is a code segment**:
    /// When unset this segment **cannot** be read.
    /// When set this segment **can** be read.
    /// **If this segment is a data segment**:
    /// When unset this segment **cannot** be written.
    /// When set this segment **can** be written to.
    #[inline]
    pub const fn set_read_write(&mut self, value: bool) -> &mut Self {
        const RW_INDEX: u8 = 1;
        const RW_MASK: u8 = 0b0000_0010;
        self.access_byte = (self.access_byte & !RW_MASK) | (value as u8) << RW_INDEX;
        self
    }

    const E_MASK: u8 = 0b0000_1000;
    /// Sets or unsets the executable bit
    /// When unset this segment is a data segment.
    /// When set this segment s a code segment.
    #[inline]
    pub const fn set_executable(&mut self, value: bool) -> &mut Self {
        const E_INDEX: u8 = 3;
        self.access_byte = (self.access_byte & !Self::E_MASK) | (value as u8) << E_INDEX;
        self
    }

    #[inline]
    #[must_use]
    pub const fn is_code(&self) -> bool {
        self.access_byte & Self::E_MASK != 0 && !self.is_system_segment()
    }

    const P_MASK: u8 = 0b1000_0000;
    /// Sets or unsets the present bit
    /// When unset this segment can not be used.
    /// When set this segment can be used.
    #[inline]
    pub const fn set_present(&mut self, value: bool) -> &mut Self {
        const P_INDEX: u8 = 7;
        self.access_byte = (self.access_byte & !Self::P_MASK) | (value as u8) << P_INDEX;
        self
    }

    #[inline]
    #[must_use]
    pub const fn present(&self) -> bool {
        self.access_byte & Self::P_MASK != 0 && !self.is_system_segment()
    }

    /// Sets or unsets the longmode  bit
    /// When unset this segment is eighter 16 or 32 bit.
    /// When set this segment is in 64 bit.
    /// If this is set the `DB` bit must be unset.
    #[inline]
    pub const fn set_longmode(&mut self, value: bool) -> &mut Self {
        const L_INDEX: u8 = 5;
        const L_MASK: u8 = 0b0010_0000;
        const DB_INDEX: u8 = 6;
        const DB_MASK: u8 = 0b0110_0000;
        self.limit_flags = (self.limit_flags & !(L_MASK | DB_MASK))
            | (value as u8) << L_INDEX
            | (!value as u8) << DB_INDEX;
        self
    }

    /// Sets or unsets the granularity  bit
    /// When unset the `limit` is in 1 byte blocks.
    /// When set the `limit` is in 4KiB pages.
    #[inline]
    pub const fn set_granularity(&mut self, value: bool) -> &mut Self {
        const G_INDEX: u8 = 7;
        const G_MASK: u8 = 0b1000_0000;
        self.limit_flags = (self.limit_flags & !G_MASK) | (value as u8) << G_INDEX;
        self
    }

    const DPL_MASK: u8 = 0b0110_0000;
    const DPL_INDEX: u8 = 5;
    /// Sets the privalage level of a segment
    /// The given privalage level and higher are only allowed to access this segment
    #[inline]
    pub const fn set_privalage_level(&mut self, level: PrivalageLevel) -> &mut Self {
        self.access_byte = (self.access_byte & !Self::DPL_MASK) | (level as u8) << Self::DPL_INDEX;
        self
    }

    /// Gets the privalage level of a segment
    ///
    /// # Panics
    /// Panics if the privalage level is higher then 4
    #[inline]
    #[must_use]
    pub const fn get_privilage_level(&self) -> PrivalageLevel {
        let level = (self.access_byte & Self::DPL_MASK) >> Self::DPL_INDEX;
        assert!(level < 4);
        match level {
            0 => PrivalageLevel::Ring0,
            1 => PrivalageLevel::Ring1,
            2 => PrivalageLevel::Ring2,
            3 => PrivalageLevel::Ring3,
            _ => panic!("Impossible"),
        }
    }
}

/// The global discriptor table
/// Readmore here : <https://wiki.osdev.org/Global_Descriptor_Table>.
#[repr(C, packed)]
pub struct GlobalDescriptorTable<const SIZE: usize>([SegmentDiscriptor; SIZE]);

/// A simple builder for the Global discriptor table
#[repr(C)]
pub struct GlobalDescriptorTableBuilder<const SIZE: usize> {
    gdt: GlobalDescriptorTable<SIZE>,
    size: u16,
}

impl<const SIZE: usize> Default for GlobalDescriptorTableBuilder<SIZE> {
    fn default() -> Self {
        Self {
            gdt: GlobalDescriptorTable([const { SegmentDiscriptor::null() }; SIZE]),
            size: 1,
        }
    }
}

impl<const SIZE: usize> GlobalDescriptorTableBuilder<SIZE> {
    /// Creates a new GDT.
    /// This also automatically appends a single null table to the GDT.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            gdt: GlobalDescriptorTable([const { SegmentDiscriptor::null() }; SIZE]),
            size: 1,
        }
    }

    /// Pushes a segment to the selector and returs the segment selector.
    #[inline]
    pub const fn append_segment_discriptor(
        &mut self,
        segment: SegmentDiscriptor,
    ) -> Option<SegmentSelector> {
        let level = segment.get_privilage_level();
        let index = self.size;
        self.size += 1;
        if self.size as usize > SIZE {
            return None;
        }
        self.gdt.0[index as usize] = segment;
        Some(SegmentSelector(index << 3 | level as u16))
    }

    #[inline]
    pub fn append_system_discriptor(
        &mut self,
        system: &SystemSegmentDiscriptor,
    ) -> Option<SegmentSelector> {
        let (part_1, part_2) = system.as_split();
        let selector = self.append_segment_discriptor(part_1)?;
        self.append_segment_discriptor(part_2)?;
        Some(selector)
    }

    pub fn find_tss(&mut self) -> Option<SegmentSelector> {
        for index in 0..SIZE {
            let access_level = self.gdt.0[index].get_privilage_level();
            if self.gdt.0[index].is_tss() && self.gdt.0[index].is_system_segment() {
                return Some(SegmentSelector(
                    u16::try_from(index).ok()? << 3 | access_level as u16,
                ));
            }
        }
        None
    }

    pub fn find_data_segment(&self, level: PrivalageLevel) -> Option<SegmentSelector> {
        for index in 0..SIZE {
            let access_level = self.gdt.0[index].get_privilage_level();
            if access_level == level
                && !self.gdt.0[index].is_code()
                && !self.gdt.0[index].is_system_segment()
            {
                return Some(SegmentSelector(
                    u16::try_from(index).ok()? << 3 | access_level as u16,
                ));
            }
        }
        None
    }

    pub fn find_code_segment(&self, level: PrivalageLevel) -> Option<SegmentSelector> {
        for index in 0..SIZE {
            let access_level = self.gdt.0[index].get_privilage_level();
            if access_level == level
                && self.gdt.0[index].is_code()
                && !self.gdt.0[index].is_system_segment()
            {
                return Some(SegmentSelector(
                    u16::try_from(index).ok()? << 3 | access_level as u16,
                ));
            }
        }
        None
    }

    /// Loads the new GDT into the GDTR
    pub fn load(&self) {
        // A structure to load data into the GDTR register
        #[derive(Debug)]
        #[repr(C, packed(2))]
        struct Gdtr {
            /// The size in bytes of the GDT
            size: u16,
            /// The virual adress of the GDT
            offset: u64,
        }

        let offset = &raw const self.gdt as u64;
        #[allow(clippy::cast_possible_truncation)]
        let size = (self.size as usize * size_of::<SegmentDiscriptor>()) as u16 - 1;
        let gdtr = &Gdtr { size, offset };
        let gdtr = core::ptr::from_ref::<Gdtr>(gdtr) as u64;
        unsafe {
            asm!("lgdt [{gdtr}]", gdtr =  in(reg) gdtr, options(readonly, nostack, preserves_flags));
        }
    }
}

/// A struct to interact with Segment selectors.
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct SegmentSelector(u16);

impl From<SegmentSelector> for usize {
    fn from(val: SegmentSelector) -> Self {
        val.0 as usize
    }
}

impl From<u16> for SegmentSelector {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl Display for SegmentSelector {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Index: {},\tTI: {}\tRPL: {}",
            (self.0 >> 3) as usize,
            if self.0 & 0x04 == 0 { "GDT" } else { "LDT" },
            unsafe { transmute::<u8, PrivalageLevel>((self.0 & 0x03) as u8) }
        )
    }
}

impl SegmentSelector {
    /// Loads loads the segment selector from the CS register.
    #[inline]
    #[must_use]
    pub fn from_cs() -> Self {
        let data: u16;
        unsafe {
            asm!("mov {data:x}, cs", data= out(reg) data, options(nostack, preserves_flags) );
        }
        Self(data)
    }

    #[inline]
    #[must_use]
    pub fn from_ss() -> Self {
        let data: u16;
        unsafe {
            asm!("mov {data:x}, ss", data= out(reg) data, options(nostack, preserves_flags) );
        }
        Self(data)
    }

    /// Saves the code selector into the CS register
    #[inline]
    pub fn set_as_cs_register(self) {
        unsafe {
            asm!(
                "push {cs:r}",
                "lea {tmp}, [55f + rip]",
                "push {tmp}",
                "retfq",
                "55:",
                cs = in(reg) self.0,
                tmp = lateout(reg) _,
                options(preserves_flags),
            );
        }
    }

    /// Saves the data selctor into the DS register
    #[inline]
    pub fn set_as_ds_register(self) {
        unsafe {
            asm!(r#"
            mov ds, {ds:r}
            mov ss, {ds:r}
            mov es, {ds:r}
            mov fs, {ds:r}
            mov gs, {ds:r}
            "#, ds = in(reg) self.0, options(nostack, preserves_flags));
        }
    }

    /// Loads the tss selector into the TR register
    #[inline]
    pub fn set_as_tss_register(self) {
        unsafe { asm!("ltr {tr:x}", tr = in(reg) self.0, options(nostack, preserves_flags)) }
    }
}
