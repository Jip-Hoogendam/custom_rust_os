use core::fmt::Display;

use alloc::{boxed::Box, format, string::String, vec};

use crate::acpi::aml::{
    AmlError, ByteCodeInstruction,
    executor::{AmlVM, ArgObj, LocalObj},
    opcodes::AmlOp,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct NameSeg([u8; 4]);

impl NameSeg {
    #[must_use]
    pub const fn root() -> Self {
        Self([0, 0, 0, 0])
    }

    #[must_use]
    pub fn from_bytes(bytes: [u8; 4]) -> Option<Self> {
        if bytes[0].is_ascii_digit() {
            return None;
        }
        for c in bytes.iter().skip(1) {
            if !(c.is_ascii_uppercase() || c.is_ascii_digit() || matches!(c, b'_')) {
                return None;
            }
        }

        Some(Self(bytes))
    }
}

impl Display for NameSeg {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}{}{}{}",
            char::from(self.0[0]),
            char::from(self.0[1]),
            char::from(self.0[2]),
            char::from(self.0[3])
        )
    }
}

impl ByteCodeInstruction for NameSeg {
    fn exec(stream: &mut AmlVM) -> Result<Self, AmlError> {
        let mut seg = [0; 4];
        for c in &mut seg {
            let byte = stream.next()?;
            if byte.is_ascii_uppercase() || byte.is_ascii_digit() || matches!(byte, b'_') {
                *c = byte;
            } else {
                Err(AmlError::InvalidData(byte, "NameSeg"))?;
            }
        }
        if seg[0].is_ascii_digit() {
            Err(AmlError::InvalidData(seg[0], "NameSeg"))
        } else {
            Ok(Self(seg))
        }
    }
}

///A path of nested name paths
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum NamePath {
    NameSeg(NameSeg),
    DualNamePath(NameSeg, NameSeg),
    MultiNamePath(Box<[NameSeg]>),
    NullName,
}

impl Display for NamePath {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::NullName => String::from("[NULL_NAME]"),
                Self::NameSeg(seg) => format!("\\\\{seg}"),
                Self::DualNamePath(seg_1, seg_2) => format!("\\\\{seg_1}.{seg_2}"),
                Self::MultiNamePath(segs) => format!(
                    "\\\\{}",
                    segs.iter().map(|seg| format!(".{seg}")).collect::<String>()
                ),
            }
        )
    }
}

impl NamePath {
    #[must_use]
    pub const fn iter(&self) -> NamePathIter<'_> {
        NamePathIter {
            namepath: self,
            index: 0,
        }
    }

    #[must_use]
    pub fn from_string(path: &str) -> Option<Self> {
        let (root, path) = path.split_at(2);
        if root != "\\\\" {
            return None;
        }
        let mut res = vec![];
        for seg in path.split('.') {
            let seg = seg.as_bytes();
            if seg.len() != 4 {
                return None;
            }
            let mut byteseg = [0u8; 4];
            for i in 0..byteseg.len() {
                byteseg[i] = seg[i];
            }
            res.push(NameSeg::from_bytes(byteseg)?);
        }
        match res.len() {
            0 => Some(Self::NullName),
            1 => Some(Self::NameSeg(res[0].clone())),
            2 => Some(Self::DualNamePath(res[0].clone(), res[1].clone())),
            3.. => Some(Self::MultiNamePath(res.into_boxed_slice())),
        }
    }
}

impl ByteCodeInstruction for NamePath {
    fn exec(ctx: &mut AmlVM) -> Result<Self, AmlError> {
        let byte = ctx.peek_opcode()?;
        Ok(match byte {
            AmlOp::NameChar => Self::NameSeg(NameSeg::exec(ctx)?),
            AmlOp::DualNameOp => {
                ctx.next();
                Self::DualNamePath(NameSeg::exec(ctx)?, NameSeg::exec(ctx)?)
            }
            AmlOp::MultiNameOp => {
                ctx.next();
                let seg_count = ctx.next()?;
                let mut name_segs = vec![];
                for _ in 0..seg_count {
                    name_segs.push(NameSeg::exec(ctx)?);
                }
                Self::MultiNamePath(name_segs.into_boxed_slice())
            }
            AmlOp::ZeroOp => {
                ctx.next();
                Self::NullName
            }
            op => Err(AmlError::UnexpectedOp(op, "NamePath"))?,
        })
    }
}

pub struct NamePathIter<'a> {
    namepath: &'a NamePath,
    index: usize,
}

impl<'a> Iterator for NamePathIter<'a> {
    type Item = &'a NameSeg;

    fn next(&mut self) -> Option<Self::Item> {
        let res = match self.namepath {
            NamePath::NameSeg(seg) if self.index == 0 => seg,
            NamePath::DualNamePath(seg, _) if self.index == 0 => seg,
            NamePath::DualNamePath(_, seg) if self.index == 1 => seg,
            NamePath::MultiNamePath(segs) => segs.get(self.index)?,
            _ => None?,
        };
        self.index += 1;
        Some(res)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameString {
    Root(NamePath),
    Rel(NamePath, usize),
}

impl ByteCodeInstruction for NameString {
    fn exec(stream: &mut AmlVM) -> Result<Self, AmlError> {
        let byte = stream.peek()?;
        Ok(match byte {
            b'A'..=b'Z' | b'_' => Self::Rel(NamePath::exec(stream)?, 0),
            b'\\' => {
                stream.next()?;
                Self::Root(NamePath::exec(stream)?)
            }
            b'^' => {
                let mut parent = 0;
                while stream.peek()? == b'^' {
                    parent += 1;
                    stream.next()?;
                }
                Self::Rel(NamePath::exec(stream)?, parent)
            }
            _ => Err(AmlError::InvalidData(byte, "NameString"))?,
        })
    }
}

pub enum SimpleName {
    NameString(NameString),
    ArgObj(ArgObj),
    LocalObj(LocalObj),
}

impl ByteCodeInstruction for SimpleName {
    fn exec(stream: &mut AmlVM) -> Result<Self, AmlError> {
        let byte = stream.peek()?;
        Ok(match byte {
            0x60..=0x67 => Self::LocalObj(LocalObj::exec(stream)?),
            0x68..=0x6E => Self::ArgObj(ArgObj::exec(stream)?),
            _ => Self::NameString(NameString::exec(stream)?),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RegionSpace {
    SystemMemory = 0x00,
    SystemIO = 0x01,
    PCIConfig = 0x02,
    EmbeddedControl = 0x03,
    SMBus = 0x04,
    SystemCMOS = 0x05,
    PciBarTarget = 0x06,
    IPMI = 0x07,
    GeneralPurposeIO = 0x08,
    GenericSerialBus = 0x09,
    PCC = 0x0A,
    OEM(u8),
}

impl RegionSpace {
    #[must_use]
    pub const fn parse(byte: u8) -> Option<Self> {
        match byte {
            0x00 => Some(Self::SystemMemory),
            0x01 => Some(Self::SystemIO),
            0x02 => Some(Self::PCIConfig),
            0x03 => Some(Self::EmbeddedControl),
            0x04 => Some(Self::SMBus),
            0x05 => Some(Self::SystemCMOS),
            0x06 => Some(Self::PciBarTarget),
            0x07 => Some(Self::IPMI),
            0x08 => Some(Self::GeneralPurposeIO),
            0x09 => Some(Self::GenericSerialBus),
            0x0A => Some(Self::PCC),
            b @ 0x80.. => Some(Self::OEM(b)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamedObjects {
    OpRegionOp {
        region_space: RegionSpace,
        offset: u64,
        length: u64,
    },
}
