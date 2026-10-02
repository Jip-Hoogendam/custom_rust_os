use core::{sync::atomic::AtomicBool, usize};

use alloc::{boxed::Box, rc::Rc, vec::Vec};

use crate::{
    acpi::aml::{
        AmlError,
        data_objects::{AmlString, Package},
        executor::{AmlVM, Buffer, ByteStream, ObjRef},
        named_objects::{NameSeg, NameString, NamedObjects},
    },
    collections::typed_arena::TypedArenaAllocation,
    serial_println,
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
    NamedObject(NamedObjects),
    Method(Method),
    String(AmlString),
    Mutex(Rc<AtomicBool>),
    Device,
    Scope,
    Package(Package),
    #[default]
    Null,
}

impl AmlData {
    pub fn invoke(self, ctx: &mut AmlVM, fn_name: &NameSeg) -> Result<AmlData, AmlError> {
        if let AmlData::Method(method) = self {
            method.invoke(ctx, fn_name)
        } else {
            Ok(self)
        }
    }
}

#[derive(Debug, Clone)]
pub struct Method {
    data: ByteStream,
    pub arg_c: u8,
    serialised: bool,
    sync_level: u8,
}

impl Method {
    #[inline]
    pub const fn new(data: ByteStream, method_flags: u8) -> Self {
        Self {
            data,
            arg_c: method_flags & 0b111,
            serialised: method_flags & 0b1000 != 0,
            sync_level: method_flags >> 4,
        }
    }

    pub fn invoke(&self, ctx: &mut AmlVM, fn_name: &NameSeg) -> Result<AmlData, AmlError> {
        let mut set = 0;
        let mut args = [const { AmlData::Null }; 7];
        for i in 0..self.arg_c {
            *args
                .get_mut(usize::from(i))
                .ok_or(AmlError::InvalidArgCount(i as usize, self.arg_c))? = ctx.exec_term_arg()?;
            set += 1;
        }

        if set < self.arg_c {
            Err(AmlError::InvalidArgCount(set as usize, self.arg_c))?;
        }

        self.call(ctx, &mut args, fn_name)
    }

    #[inline]
    pub fn call(
        &self,
        ctx: &mut AmlVM,
        args: &mut [AmlData; 7],
        fn_name: &NameSeg,
    ) -> Result<AmlData, AmlError> {
        let mut locals = [const { AmlData::Null }; 8];
        ctx.swp_args(args);
        ctx.swp_locals(&mut locals);

        let mut stream_clone = self.data.clone();
        ctx.swp_byte_stream(&mut stream_clone);
        let mut ret = AmlData::Null;
        loop {
            match ctx.exec_term_obj() {
                Ok(data) => ret = data,
                Err(AmlError::TermEnd) => break,
                Err(e) => Err(AmlError::ScopeErr(
                    fn_name.clone(),
                    Box::new(super::executor::AmlStackFrame {
                        error: e,
                        stream: ctx.stream().clone(),
                    }),
                ))?,
            }
        }
        ctx.swp_byte_stream(&mut stream_clone);

        ctx.swp_locals(&mut locals);
        ctx.swp_args(args);
        Ok(ret)
    }
}

impl AmlData {
    ///Checks if it is a interger and returns the raw value
    pub fn interger(&self) -> Result<u64, AmlError> {
        if let Self::Interger(int) = self {
            Ok(*int)
        } else {
            Err(AmlError::InvalidAmlData(self.clone(), "AmlData to int"))
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

    pub fn append_child(&mut self, child_ref: ObjRef) {
        self.children.push(child_ref);
    }

    pub fn seek_scope(&self, ctx: &AmlVM, name: &NameSeg) -> Result<ObjRef, AmlError> {
        for child in &self.children {
            let obj = ctx.get_node(*child)?;
            if *name == obj.name {
                return Ok(*child);
            }
        }

        if let Some(parent) = self.parent {
            let node = ctx.get_node(parent)?;
            serial_println!("Call!");
            node.seek_scope(ctx, name)
        } else {
            Err(AmlError::NoChildNodeBy(self.name.clone(), name.clone()))
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
