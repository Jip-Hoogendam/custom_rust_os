use alloc::boxed::Box;

use crate::acpi::aml::{
    executor::{AmlStackFrame, AmlVM, ObjRef},
    named_objects::{NamePath, NameSeg},
    namenode::AmlData,
    opcodes::{AmlOp, ExtOp},
};
use core::{error::Error, fmt::Display};

pub const AML_RECURSION_LIMIT: usize = 10;

pub mod data_objects;
pub mod executor;
pub mod named_objects;
pub mod namenode;
pub mod opcodes;
pub mod parser;
pub mod tests;

///Defines all error types by the aml executor
#[derive(Debug)]
pub enum AmlError {
    EndOfStream,
    OutOfIndex,
    NullObj,
    NullRef,
    NoTopLevelNode,
    OutOfBoundsExec,
    ScopeTooLong(usize),
    PackageTooLong(u64),
    NamedObjectNotFound(NamePath),
    InvalidOperand(AmlOp),
    InvalidExtOperand(ExtOp),
    InvalidData(u8),
    InvalidAmlData(AmlData),
    InvalidArgIndex(u8),
    InvalidLocalIndex(u8),
    InvalidObjectRef(ObjRef),
    NoChildNodeBy(NameSeg),
    RecursionLimitHit(usize),
    ScopeErr(NameSeg, Box<AmlStackFrame>),
    DebugObj,
}

impl Display for AmlError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl Error for AmlError {}

pub trait ByteCodeInstruction
where
    Self: Sized,
{
    fn exec(ctx: &mut AmlVM) -> Result<Self, AmlError>;
}

pub trait ByteCodeObject
where
    Self: Sized,
{
    fn load(ctx: &mut AmlVM) -> Result<Self, AmlError>;
}
