use core::{sync::atomic::AtomicBool, usize};

use alloc::{boxed::Box, rc::Rc, vec::Vec};

use crate::{
    acpi::aml::{
        AmlError,
        data_objects::{AmlString, Package},
        executor::{AmlVM, Buffer, ObjRef},
        named_objects::{NameSeg, NameString},
    },
    collections::typed_arena::TypedArenaAllocation,
};

///Node types that gets created or passed by the AML
#[derive(Debug, Default, Clone)]
pub enum AmlData {
    Alias(ObjRef),
    ObjRef(ObjRef),
    Buffer(Buffer),
    Interger(u64),
    Byte(u8),
    NameString(NameString),
    String(AmlString),
    Mutex(Rc<AtomicBool>),
    Device,
    Scope,
    Package(Package),
    #[default]
    Null,
}

impl AmlData {
    ///Checks if it is a interger and returns the raw value
    pub fn interger(&self) -> Result<u64, AmlError> {
        if let Self::Interger(int) = self {
            Ok(*int)
        } else {
            Err(AmlError::InvalidAmlData(self.clone()))
        }
    }
}

/// A object that gets created by the AML
pub struct NameNode {
    pub parent: Option<ObjRef>,
    pub name: NameSeg,
    pub data: AmlData,
    pub children: Vec<ObjRef>,
}

impl NameNode {
    ///Mutates the inner type. It is expected that the types are the same
    pub fn set(&mut self, data: AmlData) {
        self.data = data;
    }

    pub const fn new(name: NameSeg, data: AmlData) -> Self {
        Self {
            parent: None,
            name,
            data,
            children: Vec::new(),
        }
    }

    pub fn find_child(&self, ctx: &AmlVM, name: &NameSeg) -> Result<ObjRef, AmlError> {
        for child in &self.children {
            let obj = ctx.get_node(*child)?;
            if *name == obj.name {
                return Ok(*child);
            }
        }
        Err(AmlError::NoChildNodeBy(self.name.clone(), name.clone()))
    }
}
