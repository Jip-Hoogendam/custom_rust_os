use core::{sync::atomic::AtomicBool, usize};

use alloc::{boxed::Box, rc::Rc, vec::Vec};

use crate::{
    acpi::aml::{
        AmlError,
        data_objects::{AmlString, Package},
        executor::{AmlVM, ArgObj, Buffer, ByteStream, LocalObj, ObjRef},
        named_objects::{NameSeg, NameString, NamedObjects},
        stack_items::StackItem,
    },
    serial_println,
};

///Node types that gets created or passed by the AML
#[derive(Debug, Default, Clone)]
pub enum AmlData {
    ObjRef(ObjRef),
    Arg(ArgObj),
    Local(LocalObj),
    Index(ObjRef, usize),
    Debug,

    Buffer(Buffer),
    Interger(u64),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Method {
    data: ByteStream,
    pub arg_c: u8,
    serialised: bool,
    sync_level: u8,
}

impl Method {
    #[inline]
    #[must_use]
    pub const fn new(data: ByteStream, method_flags: u8) -> Self {
        Self {
            data,
            arg_c: method_flags & 0b111,
            serialised: method_flags & 0b1000 != 0,
            sync_level: method_flags >> 4,
        }
    }

    pub fn invoke(&self, ctx: &mut AmlVM, fn_name: &NameSeg) -> Result<(), AmlError> {
        let index = ctx.stream().index;
        let length = ctx.stream().length;

        //push locals
        for _ in 0..8 {
            ctx.stack.push(StackItem::Data(AmlData::Null))?;
        }

        ctx.stack.push(StackItem::MethodCall {
            ret_index: index,
            old_length: length,
            old_method_ptr: ctx.method_ptr,
            arg_c: self.arg_c,
        })?;
        ctx.method_ptr = ctx.stack.index();

        *ctx.stream() = self.data.clone();
        Ok(())
    }

    #[inline]
    pub fn call(
        &self,
        ctx: &mut AmlVM,
        args: &[AmlData],
        fn_name: &NameSeg,
    ) -> Result<AmlData, AmlError> {
        let mut stream_clone = self.data.clone();
        ctx.swp_byte_stream(&mut stream_clone);

        ctx.stack.push(StackItem::Eval(AmlData::Null))?;
        for arg in args {
            ctx.stack.push(StackItem::Data(arg.clone()))?;
        }
        //push locals
        for _ in 0..8 {
            ctx.stack.push(StackItem::Data(AmlData::Null))?;
        }
        ctx.stack.push(StackItem::MethodCall {
            ret_index: stream_clone.index,
            old_length: stream_clone.length,
            old_method_ptr: ctx.method_ptr,
            arg_c: self.arg_c,
        })?;
        ctx.method_ptr = ctx.stack.index();

        match ctx.exec() {
            Ok(()) => (),
            Err(e) => Err(AmlError::ScopeErr(fn_name.clone(), Box::new(e)))?,
        }

        ctx.swp_byte_stream(&mut stream_clone);

        let ret = match ctx.stack.pop()? {
            Some(StackItem::Eval(data)) => {
                serial_println!("DATA: FOUND : {:?}", data);
                data
            }
            Some(data) => Err(AmlError::StackCorruption)?,
            None => AmlData::Null,
        };
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
#[derive(Debug)]
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

    #[must_use]
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
            node.seek_scope(ctx, name)
        } else {
            Ok(ObjRef::null())
        }
    }

    pub fn has_child(&self, ctx: &AmlVM, name: &NameSeg) -> Result<bool, AmlError> {
        for child in &self.children {
            let obj = ctx.get_node(*child)?;
            if *name == obj.name {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn find_child(&self, ctx: &AmlVM, name: &NameSeg) -> Result<ObjRef, AmlError> {
        for child in &self.children {
            let obj = ctx.get_node(*child)?;
            if *name == obj.name {
                return Ok(*child);
            }
        }
        Ok(ObjRef::null())
    }
}
