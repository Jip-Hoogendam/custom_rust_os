use core::{cmp::min, mem, ops::DerefMut, sync::atomic::AtomicBool, usize};

use alloc::{boxed::Box, format, rc::Rc, vec::Vec};

use crate::{
    acpi::aml::{
        AmlError,
        data_objects::{AmlString, Buffer, Package},
        executor::{AmlVM, ArgObj, ByteStream, LocalObj, ObjRef},
        named_objects::{NameSeg, NameString, NamedObjects},
        stack_items::StackItem,
    },
    serial_println,
};

///Node types that gets created or passed by the AML
#[derive(Debug, Default, Clone)]
pub enum AmlData {
    ObjRef(ObjRef),
    StackRef(usize),
    LocalRef(usize),
    ArgRef(usize),

    NameString(NameString),
    NamedObject(NamedObjects),
    OpRegionField(),

    Scope,

    Interger(u64),         //Interger: 1
    String(AmlString),     //String: 2
    Buffer(Buffer),        //Buffer: 3
    Package(Package),      //Package: 4
    FieldUnit(),           //FieldUnit: 5
    Device(),              //Device: 6
    Event(),               //Event:7
    Method(Method),        //Control Method: 8
    Mutex(Rc<AtomicBool>), //Mutex: 9
    OpRegion(),            //Operation Region: 10
    PowerResource(),       //Power Resource: 11
    Processor(),           //Processor: 12
    ThermalZone(),         //ThermalZone: 13
    BufferField(),         //Buffer Field: 14
    Debug,                 //Debug object: 16

    #[default]
    Null, //Uninitialized: 0
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

    pub fn invoke(
        &self,
        ctx: &mut AmlVM,
        fn_ref: ObjRef,
        _fn_name: &NameSeg,
    ) -> Result<(), AmlError> {
        //push locals
        for _ in 0..8 {
            ctx.stack.push(StackItem::Data(AmlData::Null))?;
        }

        let old_method_ptr = ctx.method_ptr;
        let mut stream = self.data.clone();
        ctx.swp_byte_stream(&mut stream);

        ctx.stack.push(StackItem::MethodCall {
            prev_stream: stream,
            old_method_ptr,
            old_tlr: ctx.get_tlr(),
            arg_c: self.arg_c,
        })?;
        ctx.method_ptr = ctx.stack.index();
        ctx.set_tlr(fn_ref);

        Ok(())
    }

    #[inline]
    pub fn call(
        &self,
        fn_ref: ObjRef,
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
            prev_stream: stream_clone,
            old_method_ptr: ctx.method_ptr,
            old_tlr: ctx.get_tlr(),
            arg_c: self.arg_c,
        })?;
        ctx.method_ptr = ctx.stack.index();
        ctx.set_tlr(fn_ref);

        match ctx.exec() {
            Ok(()) => (),
            Err(e) => Err(AmlError::ScopeErr(fn_name.clone(), Box::new(e)))?,
        }

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

    pub fn concat(self, other: Self, ctx: &AmlVM) -> Result<Self, AmlError> {
        fn string_convert(data: AmlData) -> Result<AmlString, AmlError> {
            match data {
                AmlData::Package(_) => AmlString::from_bytes(b"[Package]"),
                AmlData::FieldUnit() => AmlString::from_bytes(b"[Field]"),
                AmlData::Device() => AmlString::from_bytes(b"[Device]"),
                AmlData::Event() => AmlString::from_bytes(b"[Event}"),
                AmlData::Method(_) => AmlString::from_bytes(b"[Control Method]"),
                AmlData::Mutex(_) => AmlString::from_bytes(b"[Mutex]"),
                AmlData::OpRegion() => AmlString::from_bytes(b"[Operation Region]"),
                AmlData::PowerResource() => AmlString::from_bytes(b"[Power Resource]"),
                AmlData::Processor() => AmlString::from_bytes(b"[Processor]"),
                AmlData::ThermalZone() => AmlString::from_bytes(b"[Thermal Zone]"),
                AmlData::BufferField() => AmlString::from_bytes(b"[Buffer Field]"),
                AmlData::Debug => AmlString::from_bytes(b"[Debug Object]"),
                data => Err(AmlError::InvalidAmlData(data, "String convert")),
            }
        }

        let source_1 = match self {
            r @ (Self::Interger(_) | Self::Buffer(_) | Self::String(_)) => r,
            convert => Self::String(string_convert(convert)?),
        };

        match (source_1, other) {
            (
                Self::Interger(source_1),
                r @ (Self::Buffer(_) | Self::String(_) | Self::Interger(_)),
            ) => {
                let source_2 = r.get_interger(ctx)?;
                let buffer = [source_1.to_le_bytes(), source_2.to_le_bytes()].concat();
                Ok(Self::Buffer(Buffer {
                    buffer: buffer.into_boxed_slice(),
                }))
            }
            (Self::Buffer(source_1), r) => {
                let source_2 = match r {
                    Self::Buffer(_) | Self::String(_) | Self::Interger(_) => r.buffer(ctx)?,
                    other => string_convert(other)?.to_buf(),
                };
                let buffer = [source_1.buffer, source_2.buffer].concat();
                Ok(Self::Buffer(Buffer {
                    buffer: buffer.into_boxed_slice(),
                }))
            }
            (Self::String(source_1), r) => {
                let source_2 = match r {
                    Self::Buffer(_) | Self::String(_) | Self::Interger(_) => r.string(ctx)?,
                    other => string_convert(other)?,
                };
                let source_1 = source_1.0.borrow();
                let source_2 = source_2.0.borrow();
                let buffer = [&source_1[..], &source_2[..]].concat();
                Ok(Self::String(AmlString::from_bytes(&buffer)?))
            }
            (data, _) => Err(AmlError::InvalidAmlData(data, "Concat")),
        }
    }

    ///Takes in a refence and copies the data into the location
    pub fn copy(&self, ctx: &mut AmlVM, new: AmlData) -> Result<(), AmlError> {
        match self {
            Self::LocalRef(stack_ptr) => {
                let data = ctx.get_stack_mut(*stack_ptr)?;
                *data = new;
                Ok(())
            }
            Self::ArgRef(stack_ptr) => {
                let data = ctx.get_stack_mut(*stack_ptr)?;
                let data = match data {
                    Self::StackRef(stack_ptr) => {
                        let stack_ptr = *stack_ptr;
                        ctx.get_stack_mut(stack_ptr)?
                    }
                    Self::ObjRef(obj_ref) => {
                        let obj_ref = *obj_ref;
                        &mut ctx.mut_node(obj_ref)?.data
                    }
                    data => data,
                };
                *data = new;
                Ok(())
            }
            Self::ObjRef(obj_ref) => {
                let data = ctx.mut_node(*obj_ref)?;
                data.data = new;
                Ok(())
            }
            _ => Err(AmlError::InvalidAmlData(self.clone(), "COPY")),
        }
    }

    ///Sets the new value without type conversions.
    pub fn store(&mut self, ctx: &mut AmlVM, new: AmlData) -> Result<(), AmlError> {
        let (data, new) = match self {
            Self::Null => {
                ctx.stack.push_data(new)?;
                return Ok(());
            }
            Self::Debug => {
                if !ctx.debug {
                    return Ok(());
                }
                match new {
                    Self::String(str) => serial_println!("Debug : {}", str),
                    _ => serial_println!("Debug : {:?}", new),
                }
                return Ok(());
            }
            Self::ArgRef(stack_ref) => {
                let stack_ref = *stack_ref;
                let new = new.implicit_convert(self, ctx)?;
                let data = ctx.get_stack_mut(stack_ref)?;
                (
                    match data {
                        Self::StackRef(stack_ref) => {
                            let ptr = *stack_ref;
                            ctx.get_stack_mut(ptr)?
                        }
                        Self::ObjRef(obj_ref) => {
                            let obj_ref = *obj_ref;
                            &mut ctx.mut_node(obj_ref)?.data
                        }
                        _ => {
                            *data = new;
                            return Ok(());
                        }
                    },
                    new,
                )
            }
            Self::LocalRef(stack_ref) => {
                let stack_ref = *stack_ref;

                //this one stupid edge case
                if let Ok(Self::StackRef(stack_ref)) = ctx.get_stack(stack_ref) {
                    let mut stack_ref = stack_ref;
                    let data = loop {
                        let data = ctx.get_stack(*stack_ref);
                        stack_ref = match data {
                            Ok(Self::StackRef(stack_ref)) => stack_ref,
                            Ok(data) => break data,
                            Err(e) => Err(e)?,
                        }
                    };
                    let new = new.implicit_convert(data, ctx)?;
                    let res_str = ctx.get_stack_mut(*stack_ref)?;
                    (res_str, new)
                } else {
                    let data = ctx.get_stack_mut(stack_ref)?;
                    (data, new)
                }
            }
            Self::StackRef(stack_ref) => {
                let stack_ref = *stack_ref;
                let new = new.implicit_convert(self, ctx)?;
                (ctx.get_stack_mut(stack_ref)?, new)
            }
            Self::ObjRef(obj_ref) => {
                let obj_ref = *obj_ref;
                let new = new.implicit_convert(self, ctx)?;
                (&mut ctx.mut_node(obj_ref)?.data, new)
            }
            data => Err(AmlError::InvalidAmlData(data.clone(), "Aml Store"))?,
        };

        match (data, new) {
            (Self::String(str), Self::String(res_str)) => {
                let src_str = res_str.0.borrow();
                let mut dst_str = str.0.borrow_mut();
                let len = min(dst_str.len(), src_str.len());
                for i in 0..len {
                    dst_str[i] = src_str[i];
                }
            }
            (Self::Interger(int), Self::Interger(new_int)) => *int = new_int,
            (data, new) => *data = new,
        }
        Ok(())
    }

    ///Sets the new value without type conversions.
    pub fn set(&mut self, ctx: &mut AmlVM, new: AmlData) -> Result<(), AmlError> {
        let new = new.implicit_convert(self, ctx)?;

        let data = match self {
            Self::Null => {
                ctx.stack.push_data(new)?;
                return Ok(());
            }
            Self::ArgRef(stack_ref) => {
                let data = ctx.get_stack_mut(*stack_ref)?;
                match data {
                    Self::StackRef(stack_ref) => {
                        let ptr = *stack_ref;
                        ctx.get_stack_mut(ptr)?
                    }
                    Self::ObjRef(obj_ref) => {
                        let obj_ref = *obj_ref;
                        &mut ctx.mut_node(obj_ref)?.data
                    }
                    _ => {
                        *data = new;
                        return Ok(());
                    }
                }
            }
            Self::LocalRef(stack_ref) | Self::StackRef(stack_ref) => {
                ctx.get_stack_mut(*stack_ref)?
            }
            Self::ObjRef(obj_ref) => &mut ctx.mut_node(*obj_ref)?.data,
            data => data,
        };

        match (data, new) {
            (Self::String(str), Self::String(res_str)) => {
                let src_str = res_str.0.borrow();
                for (index, byte) in str.0.borrow_mut().iter_mut().enumerate() {
                    *byte = src_str[index];
                }
            }
            (Self::Interger(int), Self::Interger(new_int)) => *int = new_int,
            (data, new) => *data = new,
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
            Self::ArgRef(arg) => ctx.get_stack_mut(*arg),
            Self::LocalRef(local) => ctx.get_stack_mut(*local),
            _ => Ok(self),
        }
    }

    ///Resolves the data to a local value
    pub fn resolve_locals<'a, 'b>(&'b self, ctx: &'a AmlVM) -> Result<&'a AmlData, AmlError>
    where
        'b: 'a,
    {
        match self {
            Self::ArgRef(arg) => ctx.get_stack(*arg),
            Self::LocalRef(local) => ctx.get_stack(*local),
            _ => Ok(self),
        }
    }

    ///Checks if it is a interger and returns the raw value. If it is not a interger it will
    ///choherse it into a interger.
    pub fn get_interger(&self, ctx: &AmlVM) -> Result<u64, AmlError> {
        let to_int = self.resolve_locals(ctx)?.deref(ctx)?;
        match to_int {
            Self::Interger(int) => Ok(*int),
            Self::String(str)
                if !str.0.borrow().is_empty()
                    && matches!(str.0.borrow()[0], b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F') =>
            {
                let mut buffer = 0u64;
                let len = min(str.len() - 3, 16);
                for (index, nibble) in str.0.borrow().iter().enumerate().take(16) {
                    let index = (len - index) * 4;
                    buffer |= match nibble {
                        d @ b'0'..=b'9' => u64::from(d - b'0') << index,
                        c @ b'a'..=b'f' => u64::from(c - b'a' + 10) << index,
                        c @ b'A'..=b'F' => u64::from(c - b'A' + 10) << index,
                        _ => {
                            buffer >>= index;
                            break;
                        }
                    }
                }
                Ok(buffer)
            }

            Self::String(str) => {
                let mut new_int = 0x0u64;
                for (index, char) in str.0.borrow().iter().take(8).enumerate() {
                    new_int |= u64::from(*char) << (index * 8);
                }
                Ok(new_int)
            }

            Self::Buffer(Buffer { buffer }) if buffer.iter().len() <= size_of::<u64>() => {
                let mut bytes = [0; size_of::<u64>()];
                for i in 0..buffer.len() {
                    bytes[i] = buffer[i];
                }
                let int = u64::from_le_bytes(bytes);
                Ok(int)
            }
            data => Err(AmlError::InvalidAmlData(data.clone(), "AmlData to int")),
        }
    }

    ///Checks if is a string and returns the raw value. If it is not a string it will coherse it
    ///into a string.
    pub fn string(&self, ctx: &AmlVM) -> Result<AmlString, AmlError> {
        match self {
            Self::String(string) => Ok(string.clone()),
            Self::Interger(int) => AmlString::from_bytes(&int.to_le_bytes()),
            Self::Buffer(buf) => AmlString::from_bytes(&buf.buffer),

            data => Err(AmlError::InvalidAmlData(data.clone(), "AmlData to string")),
        }
    }

    ///Checks if it is a buffer and returns the raw value. If it is not a buffer it will try to
    ///convert it into a buffer
    pub fn buffer(&self, ctx: &AmlVM) -> Result<Buffer, AmlError> {
        match self {
            Self::Buffer(buf) => Ok(buf.clone()),
            Self::Interger(int) => Ok(Buffer {
                buffer: Box::new(int.to_le_bytes()),
            }),
            Self::String(string) => Ok(string.clone().to_buf()),
            data => Err(AmlError::InvalidAmlData(data.clone(), "AmlData to string")),
        }
    }

    pub fn implicit_convert(self, to: &Self, ctx: &AmlVM) -> Result<AmlData, AmlError> {
        //Find what to convert to.
        let mut to = to;
        loop {
            match to {
                Self::StackRef(stack_ref) => to = ctx.get_stack(*stack_ref)?,
                Self::ObjRef(obj_ref) => to = &ctx.get_node(*obj_ref)?.data,
                Self::ArgRef(_) => {
                    return Ok(self);
                }
                Self::LocalRef(data_ref)
                    if matches!(
                        ctx.get_stack(*data_ref),
                        Ok(Self::StackRef(_) | Self::ObjRef(_))
                    ) =>
                {
                    to = ctx.get_stack(*data_ref)?;
                }
                _ => break,
            }
        }

        //Deref self
        let mut data = self;
        loop {
            match data {
                Self::StackRef(stack_ref) => data = ctx.get_stack(stack_ref)?.clone(),
                Self::ObjRef(obj_ref) => data = ctx.get_node(obj_ref)?.data.clone(),
                _ => break,
            }
        }

        //Convert
        match to {
            Self::Interger(_) => Ok(Self::Interger(data.get_interger(ctx)?)),
            Self::String(string)
                if string.0.borrow().is_empty()
                    && let Self::Interger(int) = data =>
            {
                AmlString::from_bytes(format!("{int:X}").as_bytes()).map(AmlData::String)
            }
            Self::String(_) => Ok(Self::String(data.string(ctx)?)),
            _ => Ok(data),
        }
    }

    ///Changes the lhs of the data while applying the implicit type conversions.
    pub fn mutate(&mut self, other: Self, ctx: &mut AmlVM) -> Result<(), AmlError> {
        if matches!(self, AmlData::Null) {
            ctx.stack.push_data(other)?;
            return Ok(());
        }
        let converted = match other {
            Self::ArgRef(arg) => {
                let data = ctx.get_stack(arg)?;
                match data {
                    Self::StackRef(stack_ref) => ctx.get_stack(*stack_ref)?,
                    Self::ObjRef(obj_ref) => &mut ctx.mut_node(*obj_ref)?.data,
                    _ => data,
                }
                .clone()
                .implicit_convert(self, ctx)?
            }
            Self::LocalRef(local) => {
                let data = ctx.get_stack(local)?;
                match data {
                    Self::StackRef(stack_ref) => ctx.get_stack(*stack_ref)?,
                    Self::ObjRef(obj_ref) => &mut ctx.mut_node(*obj_ref)?.data,
                    data => data,
                }
                .clone()
                .implicit_convert(self, ctx)?
            }
            _ => other.implicit_convert(self, ctx)?,
        };

        let mut target = self;
        loop {
            target = match target {
                Self::ArgRef(stack_ref) | Self::LocalRef(stack_ref) | Self::StackRef(stack_ref) => {
                    let stack_ref = *stack_ref;
                    ctx.get_stack_mut(stack_ref)?
                }
                Self::ObjRef(obj_ref) => {
                    let obj_ref = *obj_ref;
                    &mut ctx.mut_node(obj_ref)?.data
                }
                _ => break,
            };
        }
        *target = converted;
        Ok(())
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
