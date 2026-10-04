use core::ops::Add;

use alloc::{boxed::Box, vec};

use crate::acpi::aml::{
    AmlError, ByteCodeInstruction, Resolvable,
    executor::{self, AmlVM, Buffer},
    named_objects::NameString,
    namenode::AmlData,
};

#[derive(Debug, Clone)]
pub struct AmlString(pub Box<[u8]>);

impl AmlString {
    pub fn exec(ctx: &mut AmlVM) -> Result<Self, AmlError> {
        let mut string = vec![];
        loop {
            match ctx.next()? {
                0x00 => break Ok(Self(string.into_boxed_slice())),
                char @ 0x01..0x7F => string.push(char),
                data => break Err(AmlError::InvalidData(data, "AmlString")),
            }
        }
    }
}

#[repr(u8)]
#[derive(Debug)]
pub enum DataObj {
    ByteConst(u8),
    WordConst(u16),
    DWordConst(u32),
    QWordConst(u64),
    String(AmlString),
    ConstZeroObj,
    ConstOneObj,
    ConstOnesObj,
    Buffer(Buffer),
    Package(Package),
    NameString(NameString),
}

pub fn load_byte(stream: &mut AmlVM) -> Result<u8, AmlError> {
    stream.next()
}

pub fn load_word(ctx: &mut AmlVM) -> Result<u16, AmlError> {
    Ok(u16::from_le_bytes([load_byte(ctx)?, load_byte(ctx)?]))
}

pub fn load_dword(ctx: &mut AmlVM) -> Result<u32, AmlError> {
    Ok(u32::from_le_bytes([
        load_byte(ctx)?,
        load_byte(ctx)?,
        load_byte(ctx)?,
        load_byte(ctx)?,
    ]))
}

pub fn load_qword(ctx: &mut AmlVM) -> Result<u64, AmlError> {
    Ok(u64::from_le_bytes([
        load_byte(ctx)?,
        load_byte(ctx)?,
        load_byte(ctx)?,
        load_byte(ctx)?,
        load_byte(ctx)?,
        load_byte(ctx)?,
        load_byte(ctx)?,
        load_byte(ctx)?,
    ]))
}

#[derive(Debug, Clone)]
pub struct Package {
    elements: Box<[AmlData]>,
}

impl Resolvable for Package {
    fn load(ctx: &mut AmlVM, start: usize) -> Result<Self, AmlError> {
        let len = ctx.pop_data()?.interger()?;
        let elements = ctx.pop_data()?.interger()?;
        let strea_save = ctx.stream().index();
        ctx.stream().index = start;
        let elements = ctx.fn_for(
            elements as usize,
            len as usize,
            executor::AmlVM::exec_package_element,
        )?;
        ctx.stream().index = strea_save;
        Ok(Self { elements })
    }
}
