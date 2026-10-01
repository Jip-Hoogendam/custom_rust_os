use core::fmt::Display;

use alloc::{boxed::Box, format, string::String, vec};

use crate::acpi::aml::{
    AmlError, ByteCodeInstruction,
    executor::{AmlVM, ArgObj, Buffer, LocalObj, ObjRef},
    opcodes::AmlOp,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct NameSeg([u8; 4]);

impl NameSeg {
    pub fn root() -> Self {
        Self([0, 0, 0, 0])
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
                Err(AmlError::InvalidData(byte))?;
            }
        }
        if seg[0].is_ascii_digit() {
            Err(AmlError::InvalidData(seg[0]))
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
                Self::NameSeg(seg) => format!("./{}", seg),
                Self::DualNamePath(seg_1, seg_2) => format!("./{}/{}", seg_1, seg_2),
                Self::MultiNamePath(segs) => format!(
                    ".{}",
                    segs.iter()
                        .map(|seg| format!("/{}", seg))
                        .collect::<String>()
                ),
            }
        )
    }
}

impl NamePath {
    pub fn iter(&self) -> NamePathIter {
        NamePathIter {
            namepath: self,
            index: 0,
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
            AmlOp::MultiplyOp => {
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
            _ => Err(AmlError::InvalidData(byte))?,
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
