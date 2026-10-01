use alloc::{boxed::Box, format, string::String};

use crate::acpi::aml::{
    executor::{AmlStackFrame, AmlVM, ObjRef},
    named_objects::{NamePath, NameSeg},
    namenode::AmlData,
    opcodes::{AmlOp, AnyAmlOp, ExtOp},
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
    NullObj,
    NullRef,
    OutOfBoundsExec,
    ScopeTooLong(usize),
    PackageTooLong(u64),
    NamedObjectNotFound(NamePath),

    InvalidOperand(u8),
    InvalidExtOperand(u8),
    InvalidData(u8),
    InvalidAmlData(AmlData),
    InvalidArgIndex(u8),
    InvalidLocalIndex(u8),
    InvalidObjectRef(ObjRef),

    UnexpectedOp(AmlOp, &'static str),
    UnexpectedExtOp(ExtOp, &'static str),

    NoChildNodeBy(NameSeg, NameSeg),
    RecursionLimitHit(usize),
    ScopeErr(NameSeg, Box<AmlStackFrame>),
    TermEnd,
}

impl Display for AmlError {
    #[rustfmt::skip]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::EndOfStream => String::from("EndOfStream: Stream ended unexpectedly"),
                Self::NullObj => String::from("NullObj: AmlVM tried to refrence a non existant object"),
                Self::NullRef => String::from("NullRef: AmlVM tried to refrence a null refence"),
                Self::OutOfBoundsExec => String::from("OutOfBoundsExec: AmlVM tired to execute on data that is out of bounds"),
                Self::ScopeTooLong(size) => format!("ScopeTooLong: Bytecode requested a scope of {size} Bytes but that is longer then end of the the aml code"),
                Self::PackageTooLong(size) => format!("PackageTooLong: Bytecode requested a package of {size} Bytes but that is longer then the end of the aml code"),
                Self::NamedObjectNotFound(path) => format!("NamedObjectNotFound: Bytecode requested a named object at {path} but this does not exist."),
                Self::InvalidOperand(op) => format!("InvalidOperand: AmlVM tried to decode non defined operand, found op : {op:#0x}"),
                Self::InvalidExtOperand(extop) => format!("InvalidExtOperand: AmlVM tried to decode a undefined ext operand, found extop : {extop:#0x}"),
                Self::InvalidData(byte) => format!("InvalidData: AmlVM failed to decode something as the passed data was invalid failed on byte : {byte:#0x}"),
                Self::InvalidAmlData(data) => format!("InvalidAmlData: AmlVM expected the data to be of a diffent type, found data: {data:?}"),
                Self::InvalidArgIndex(index) => format!("InvalidArgIndex: Arg at {index} is not defined in AmlVM"),
                Self::InvalidLocalIndex(index) => format!("InvalidLocalIndex: Local at {index} is not defined in AmlVM"),
                Self::InvalidObjectRef(obj_ref) => format!("InvalidOpjectRef: No object exists at {} in AmlVM", obj_ref.usize()),
                Self::UnexpectedOp(op,while_parse) => format!("UnexpectedOp: AmlVM found unexpected op {op:?} while parseing {while_parse}"),
                Self::UnexpectedExtOp(op,while_parse) => format!("UnexpectedExtOp: AmlVM found unexpected ext op {op:?} while parseing {while_parse}"),
                Self::NoChildNodeBy(obj, name) => format!("NoChildNodeBy: AmlVM found no child named {name} in {obj}"),
                Self::RecursionLimitHit(limit) => format!("RecursionLimitHit: AmlVM hit its recursion limit of {limit}" ),
                Self::ScopeErr(scope, stack_frame) => format!("inside: {} at index {}\n{}", scope, stack_frame.index, stack_frame.error),
                Self::TermEnd => String::from("TermEnd: AmlVM finised correctly"),
            }
        )
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
