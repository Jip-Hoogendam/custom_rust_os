use core::{ops::DerefMut, sync::atomic::AtomicBool};

use alloc::{boxed::Box, format, rc::Rc, vec::Vec};

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
    Local(LocalObj),
    Arg(ArgObj),
    ObjRef(ObjRef),
    StackRef(usize),
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

    pub fn invoke(&self, ctx: &mut AmlVM, _fn_name: &NameSeg) -> Result<(), AmlError> {
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
            Some(_data) => Err(AmlError::StackCorruption)?,
            None => AmlData::Null,
        };
        Ok(ret)
    }
}

impl AmlData {
    pub fn deref<'a>(&'a self, ctx: &'a AmlVM) -> Result<&'a Self, AmlError> {
        let mut obj = self;
        loop {
            obj = match obj {
                Self::StackRef(stack_ref) => ctx.get_stack(*stack_ref)?,
                Self::ObjRef(obj_ref) => &ctx.get_node(*obj_ref)?.data,
                _ => break Ok(obj),
            }
        }
    }

    pub fn deref_mut<'a, 'b, 'c>(&'a mut self, ctx: &'b mut AmlVM) -> Result<&'c mut Self, AmlError>
    where
        'a: 'c,
        'b: 'c,
    {
        match self {
            Self::StackRef(stack_ref) => ctx.get_stack_mut(*stack_ref),
            Self::ObjRef(obj_ref) => Ok(&mut ctx.mut_node(*obj_ref)?.data),
            _ => Ok(self),
        }
    }

    ///Sets the new value without type conversions.
    pub fn set(&mut self, ctx: &mut AmlVM, new: AmlData) -> Result<(), AmlError> {
        match self {
            Self::Null => ctx.stack.push_data(new)?,
            Self::Arg(arg) => {
                let data = ctx.get_arg_ref(*arg)?;
                let data = ctx.get_stack_mut(data)?;
                let data_ref = match data {
                    Self::StackRef(stack_ref) => {
                        let ptr = stack_ref.clone();
                        ctx.get_stack_mut(ptr)?
                    }
                    Self::ObjRef(obj_ref) => {
                        let obj_ref = obj_ref.clone();
                        &mut ctx.mut_node(obj_ref)?.data
                    }
                    _ => data,
                };
                *data_ref = new;
                return Ok(());
            }
            Self::Local(local) => {
                let data = ctx.get_local_ref(*local)?;
                let data = ctx.get_stack_mut(data)?;
                *data = new;
                return Ok(());
            }
            data => *data = new,
        }
        Ok(())
    }

    ///Resolves the data to a local value mutablily
    pub fn resolve_locals_mut<'a, 'b>(
        &'b mut self,
        ctx: &'a mut AmlVM,
    ) -> Result<&'a mut AmlData, AmlError>
    where
        'b: 'a,
    {
        match self {
            Self::Arg(arg) => {
                let data = ctx.get_arg_ref(*arg)?;
                ctx.get_stack_mut(data)
            }
            Self::Local(local) => {
                let data = ctx.get_local_ref(*local)?;
                ctx.get_stack_mut(data)
            }
            _ => Ok(self),
        }
    }

    ///Resolves the data to a local value
    pub fn resolve_locals<'a, 'b>(&'b self, ctx: &'a AmlVM) -> Result<&'a AmlData, AmlError>
    where
        'b: 'a,
    {
        match self {
            Self::Arg(arg) => {
                let data = ctx.get_arg_ref(*arg)?;
                ctx.get_stack(data)
            }
            Self::Local(local) => {
                let data = ctx.get_local_ref(*local)?;
                ctx.get_stack(data)
            }
            _ => Ok(self),
        }
    }

    ///Checks if it is a interger and returns the raw value. If it is not a interger it will
    ///choherse it into a interger.
    pub fn interger(&self, ctx: &AmlVM) -> Result<u64, AmlError> {
        match self {
            Self::Interger(int) => Ok(*int),
            r @ (Self::Arg(_) | Self::Local(_)) => r.resolve_locals(ctx)?.interger(ctx),
            Self::String(str)
                if !str.0.is_empty()
                    && matches!(str.0[0], b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F') =>
            {
                let mut buffer = 0u64;
                for (index, nibble) in str.0.iter().enumerate().take(16) {
                    buffer |= match nibble {
                        d @ b'0'..=b'9' => u64::from(d - b'0') << (16 - index),
                        c @ b'a'..=b'f' => u64::from(c - b'a' + 10) << (16 - index),
                        c @ b'A'..=b'F' => u64::from(c - b'A' + 10) << (16 - index),
                        _ => break,
                    }
                }
                Ok(buffer)
            }

            data => Err(AmlError::InvalidAmlData(data.clone(), "AmlData to int")),
        }
    }

    ///Checks if is a string and returns the raw value. If it is not a string it will coherse it
    ///into a string.
    pub fn string(&self, ctx: &AmlVM) -> Result<AmlString, AmlError> {
        match self {
            Self::String(string) => Ok(string.clone()),
            Self::Interger(int) => AmlString::from_bytes(format!("{int:X}").as_bytes()),

            data => Err(AmlError::InvalidAmlData(data.clone(), "AmlData to string")),
        }
    }

    pub fn implicit_convert(self, to: &Self, ctx: &AmlVM) -> Result<AmlData, AmlError> {
        match to {
            Self::Interger(_) => Ok(AmlData::Interger(self.interger(ctx)?)),
            Self::String(_) => Ok(AmlData::String(self.string(ctx)?)),
            _ => Ok(self),
        }
    }

    ///Changes the lhs of the data while applying the implicit type conversions.
    pub fn mutate(&mut self, other: Self, ctx: &mut AmlVM) -> Result<(), AmlError> {
        let converted = match other {
            Self::Arg(arg) => {
                let data = ctx.get_arg_ref(arg)?;
                let data = ctx.get_stack(data)?;
                match data {
                    Self::StackRef(stack_ref) => {
                        let ptr = stack_ref.clone();
                        ctx.get_stack(ptr)?
                    }
                    Self::ObjRef(obj_ref) => {
                        let obj_ref = obj_ref.clone();
                        &mut ctx.mut_node(obj_ref)?.data
                    }
                    _ => data,
                }
                .clone()
                .implicit_convert(self, ctx)?
            }
            Self::Local(local) => {
                let data = ctx.get_local_ref(local)?;
                ctx.get_stack(data)?.clone().implicit_convert(self, ctx)?
            }
            _ => other.implicit_convert(self, ctx)?,
        };
        self.set(ctx, converted)
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
