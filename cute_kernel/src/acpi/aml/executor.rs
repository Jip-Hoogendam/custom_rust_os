use alloc::vec;
use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec::Vec,
};
use core::cmp::{max, min};
use core::{
    error::Error,
    fmt::Display,
    ops::{BitAnd, BitOr, BitXor},
    ptr::null,
};

use super::stack_items::StackItem;

use crate::{
    acpi::{
        SDTables,
        aml::{
            AmlError, ByteCodeInstruction, LOCAL_COUNT,
            data_objects::{
                AmlString, Buffer, Package, load_byte, load_dword, load_qword, load_word,
            },
            named_objects::{NamePath, NameSeg, NameString},
            namenode::{AmlData, Method, NameNode},
            opcodes::{AmlOp, ExtOp},
            stack_items::AmlStack,
        },
    },
    collections::typed_arena::{TypedArena, TypedArenaAllocation},
    memory::addresses::VirtAddr,
    print, serial_println,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ByteStream {
    bytes: *const u8,
    pub index: usize,
    pub length: usize,
}

impl Display for ByteStream {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "range:{}->{} next 5 bytes : {:#0x},{:#0x},{:#0x},{:#0x},{:#0x}",
            self.index,
            self.length,
            unsafe { *self.bytes.add(self.index) },
            unsafe { *self.bytes.add(self.index + 1) },
            unsafe { *self.bytes.add(self.index + 2) },
            unsafe { *self.bytes.add(self.index + 3) },
            unsafe { *self.bytes.add(self.index + 4) },
        )
    }
}

impl ByteStream {
    #[must_use]
    #[inline]
    pub const fn index(&self) -> usize {
        self.index
    }

    #[must_use]
    #[inline]
    pub const fn new(start_point: *const u8, length: usize) -> Self {
        Self {
            bytes: start_point,
            index: 0,
            length,
        }
    }

    #[inline]
    #[must_use]
    pub fn substream_untill(&self, end: usize) -> Option<Self> {
        if end <= self.length {
            let mut clone = self.clone();
            clone.length = core::cmp::min(end, self.length);
            Some(clone)
        } else {
            None
        }
    }

    #[inline]
    #[must_use]
    pub const fn at(&self, index: usize) -> Option<u8> {
        if index >= self.length {
            None
        } else {
            Some(unsafe { *self.bytes.add(index) })
        }
    }

    pub const fn peek(&mut self) -> Option<u8> {
        self.at(self.index)
    }

    #[must_use]
    pub const fn done(&self) -> bool {
        self.index >= self.length
    }
}

impl Iterator for ByteStream {
    type Item = u8;

    ///Retruns the next byte in the stream
    fn next(&mut self) -> Option<Self::Item> {
        let byte = self.at(self.index)?;
        self.index += 1;
        Some(byte)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArgObj(u8);
impl ArgObj {
    fn parse(op: AmlOp) -> Result<Self, AmlError> {
        let op = op as u8;
        let index = if matches!(op, b'h'..=b'n') {
            op - b'h'
        } else {
            Err(AmlError::InvalidData(op, "LocalObj"))?
        };
        Ok(Self(index))
    }
}

impl ByteCodeInstruction for ArgObj {
    fn exec(stream: &mut AmlVM) -> Result<Self, AmlError> {
        let op = stream.next_opcode()?;
        Self::parse(op)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalObj(u8);
impl LocalObj {
    fn parse(op: AmlOp) -> Result<Self, AmlError> {
        let op = op as u8;
        let index = if matches!(op, b'`'..=b'g') {
            op - b'`'
        } else {
            Err(AmlError::InvalidData(op, "LocalObj"))?
        };
        Ok(Self(index))
    }
}

impl ByteCodeInstruction for LocalObj {
    fn exec(stream: &mut AmlVM) -> Result<Self, AmlError> {
        let op = stream.next_opcode()?;
        Self::parse(op)
    }
}

pub type ObjRef = TypedArenaAllocation;

#[derive(Debug)]
pub struct AmlStackFrame {
    pub error: AmlError,
    pub stack: Vec<StackItem>,
    pub stream: ByteStream,
}

impl Error for AmlStackFrame {}

impl Display for AmlStackFrame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "[AML EXEC ERROR]\n{}\nStream : {}\nbacktrace:\n{}",
            self.stack
                .iter()
                .map(|i| format!("{i:?}"))
                .fold(String::new(), |c, b| format!("{c}\n{b}")),
            self.stream,
            self.error
        )
    }
}

///Aml parser and exectuor
pub struct AmlVM {
    stream: ByteStream,
    top_level_node: ObjRef,
    pub stack: AmlStack,
    heap: TypedArena<NameNode>,
    pub method_ptr: usize,
}

impl AmlVM {
    ///Crates a new AML vm to process aml data
    ///
    /// # Panics
    /// Never i just don't want to disable the clippy
    #[must_use]
    #[rustfmt::skip]
    pub fn new(stream: ByteStream, stack_size: usize) -> Self {
        let mut heap = TypedArena::new();
        let root_obj = NameNode::new(NameSeg::root(), AmlData::Null);
        let root_ref = heap.instert(root_obj);

        let mut new = Self {
            stream,
            heap,
            top_level_node: root_ref,
            stack: AmlStack::new(stack_size),
            method_ptr: 0,
        };

        //append default nodes.
        new.append_to_node(root_ref, NameNode::new(NameSeg::from_bytes(*b"_TZ_").unwrap(), AmlData::Null)).unwrap();
        new.append_to_node(root_ref, NameNode::new(NameSeg::from_bytes(*b"_SI_").unwrap(), AmlData::Null)).unwrap();
        new.append_to_node(root_ref, NameNode::new(NameSeg::from_bytes(*b"_SB_").unwrap(), AmlData::Null)).unwrap();
        new.append_to_node(root_ref, NameNode::new(NameSeg::from_bytes(*b"_PR_").unwrap(), AmlData::Null)).unwrap();
        new.append_to_node(root_ref, NameNode::new(NameSeg::from_bytes(*b"_GPE").unwrap(), AmlData::Null)).unwrap();

        new
    }

    ///Executes from the current stream pointer.
    ///
    /// # Errors
    /// Bubbles up any error the vm encountered while its processor.
    pub fn exec(&mut self) -> Result<(), AmlStackFrame> {
        loop {
            match self.stack.pop_unresolved() {
                Ok(Some(item @ StackItem::Eval(_))) => {
                    self.stack.push(item);
                    break;
                }
                Ok(Some(StackItem::PackageElement)) if self.stream.done() => {
                    match self.stack.pop_scoped() {
                        Ok(Some(StackItem::Package {
                            old_length,
                            package,
                            ..
                        })) => {
                            self.stream.length = old_length;
                            self.stack.push_data(AmlData::Package(package))
                        }
                        Err(e) => Err(self.unwind_stackframe(e))?,
                        _ => Err(self.unwind_stackframe(AmlError::NotInMethod))?,
                    }
                }
                Ok(Some(item)) => self.resolve_stack_item(item),

                Ok(None) => {
                    if self.stream.done() {
                        match self.stack.pop_scoped() {
                            Ok(Some(StackItem::Package {
                                old_length,
                                package,
                                ..
                            })) => {
                                self.stream.length = old_length;
                                self.stack.push_data(AmlData::Package(package));
                            }

                            Ok(Some(StackItem::Load {
                                prev_stream,
                                mut target,
                            })) => {
                                self.stream = prev_stream;
                                target
                                    .store(self, AmlData::Interger(1))
                                    .map_err(|e| self.unwind_stackframe(e))?;
                            }
                            Ok(Some(StackItem::Conditional {
                                old_length,
                                ret_index,
                            })) => {
                                self.stream.index = ret_index;
                                self.stream.length = old_length;
                            }
                            Ok(Some(StackItem::Scope {
                                old_length,
                                old_tlr,
                            })) => {
                                self.stream.length = old_length;
                                self.top_level_node = old_tlr;
                            }
                            Ok(Some(StackItem::MethodCall {
                                mut prev_stream,
                                old_method_ptr,
                                old_tlr,
                                arg_c,
                            })) => {
                                self.swp_byte_stream(&mut prev_stream);
                                self.method_ptr = old_method_ptr;
                                self.set_tlr(old_tlr);
                                //pop locals and args
                                for _ in 0..(8 + arg_c) {
                                    self.stack.pop();
                                }
                                self.stack.push_data(AmlData::Null);
                            }
                            Ok(None) => break,
                            Ok(_) => Err(self.unwind_stackframe(AmlError::StackCorruption))?,
                            Err(e) => Err(self.unwind_stackframe(e))?,
                        }
                    } else {
                        serial_println!("Call ?");
                        self.exec_term_obj()
                            .map_err(|e| self.unwind_stackframe(e))?;
                    }
                    Ok(())
                }
                Err(e) => Err(e),
            }
            .map_err(|e| self.unwind_stackframe(e))?;
        }
        Ok(())
    }

    fn unwind_stackframe(&mut self, err: AmlError) -> AmlStackFrame {
        let mut stack = vec![];

        while let Ok(Some(item)) = self.stack.pop() {
            stack.push(item);
        }

        AmlStackFrame {
            error: err,
            stack,
            stream: self.stream.clone(),
        }
    }

    #[inline]
    pub fn resolve_stack_item(&mut self, item: StackItem) -> Result<(), AmlError> {
        match item {
            StackItem::Byte => {
                let data = AmlData::Interger(u64::from(load_byte(self)?));
                self.stack.push_data(data)
            }
            StackItem::NameString => {
                let data = AmlData::NameString(NameString::exec(self)?);
                self.stack.push_data(data)
            }

            StackItem::PackageElement => {
                let op = self.peek_opcode()?;
                // !TODO add buffer filed refrence and field refrence.
                match op {
                    AmlOp::MultiNameOp
                    | AmlOp::NameOp
                    | AmlOp::DualNameOp
                    | AmlOp::NameChar
                    | AmlOp::Rootchar
                    | AmlOp::ParentPrefixChar => {
                        if let Ok(obj_ref) = self.resolve_name_string()
                            && !obj_ref.is_null()
                        {
                            let node = self.get_node(obj_ref)?;
                            let data = match &node.data {
                                r @ (AmlData::Interger(_)
                                | AmlData::String(_)
                                | AmlData::Buffer(_)
                                | AmlData::Package(_)) => r,
                                _ => &AmlData::ObjRef(obj_ref),
                            };

                            self.stack.push_data(data.clone())
                        } else {
                            self.stack.push_data(AmlData::Null)
                        }
                    }
                    _ => {
                        let op_code = self.next_opcode()?;
                        self.parse_data_ref_object(op_code)
                    }
                }
            }
            StackItem::DataRefObj => {
                let op = self.next_opcode()?;
                self.parse_data_ref_object(op)
            }
            StackItem::SimpleName => self.resolve_simple_name(),
            StackItem::SuperName => self.resolve_super_name(),
            StackItem::Target => self.resolve_target(),
            StackItem::TermArg => self.exec_term_arg(),
            StackItem::ResExtOp(index) => self.resolve_ext_op(index),
            StackItem::ResOp(index) => self.resolve_op(index),
            StackItem::Pkglen => {
                let data = AmlData::Interger(self.decode_pkg_length()? as u64);
                self.stack.push_data(data)
            }
            StackItem::Return => {
                let data = self.stack.pop_data()?;
                self.stack.push_data(data)
            }
            item => {
                todo!("All possible stack items {:?} ", item)
            }
        }
    }

    pub fn resolve_ext_op(&mut self, op_index: usize) -> Result<(), AmlError> {
        serial_println!("ext op : {:?}", self.ext_op_at(op_index));
        match self.ext_op_at(op_index)? {
            ExtOp::CondRefOfOp => {
                let data = self.stack.pop_data()?;
                let mut target = self.stack.pop_data()?;
                let data = match data {
                    AmlData::ArgRef(stack_ref) | AmlData::LocalRef(stack_ref) => {
                        self.stack.push_data(AmlData::Interger(1))?;
                        AmlData::StackRef(stack_ref)
                    }
                    r @ AmlData::ObjRef(obj_ref) if !obj_ref.is_null() => {
                        self.stack.push_data(AmlData::Interger(1))?;
                        r
                    }
                    _ => {
                        self.stack.push_data(AmlData::Interger(0))?;
                        return Ok(()); // Dont push anything
                    }
                };
                target.store(self, data)
            }
            ExtOp::LoadOp => {
                let AmlData::NameString(name_string) = self.stack.pop_data()? else {
                    return Ok(());
                };

                let mut target = self.stack.pop_data()?;
                let obj_ref = self.find_node_string(name_string)?;
                let obj = self.get_node(obj_ref)?;
                let ptr = match &obj.data {
                    AmlData::Buffer(buffer) => buffer.buffer.as_ptr(),
                    AmlData::OpRegion() | AmlData::OpRegionField() => {
                        todo!("Implement op region fields")
                    }
                    data => Err(AmlError::InvalidAmlData(data.clone(), "LoadOp"))?,
                };

                let adress = VirtAddr::from_ptr(ptr);

                if let Ok(SDTables::SSDT(ssdt)) = SDTables::load(adress) {
                    let mut stream = ssdt.load_byte_stream();
                    core::mem::swap(&mut stream, self.stream());
                    self.stack.push(StackItem::Load {
                        prev_stream: stream,
                        target,
                    })
                } else {
                    target.store(self, AmlData::Interger(0))
                }
            }
            op => Err(AmlError::UnexpectedExtOp(op, "Resolve ext op")),
        }
    }

    pub fn resolve_op(&mut self, op_index: usize) -> Result<(), AmlError> {
        serial_println!("op : {:?}", self.op_at(op_index));
        match self.op_at(op_index)? {
            AmlOp::NameOp => {
                let name = self.pop_data()?;
                if let AmlData::NameString(name) = name {
                    let data = self.pop_data()?;
                    self.push_node(name, data)
                } else {
                    Err(AmlError::InvalidAmlData(name, "ResolveOp Name op"))
                }
            }

            AmlOp::DerefOfOp => {
                let data = self.pop_data()?;
                let res = match data {
                    AmlData::StackRef(stack_ref)
                    | AmlData::ArgRef(stack_ref)
                    | AmlData::LocalRef(stack_ref) => self.get_stack(stack_ref)?,
                    AmlData::ObjRef(obj_ref) => &self.get_node(obj_ref)?.data,
                    AmlData::String(string) => &AmlData::String(AmlString::from_bytes(
                        string.0.borrow().iter().as_slice(),
                    )?),
                    _ => Err(AmlError::InvalidAmlData(data, "Dref of op"))?,
                };
                self.stack.push_data(res.clone())
            }

            AmlOp::StoreOp => {
                let data = self.pop_data()?;
                let data = data.resolve_locals(self)?.clone();

                let mut name_res = self.pop_data()?;
                name_res.store(self, data)
            }

            AmlOp::WhileOp => {
                let len = self.pop_data()?.get_interger(self)? as usize;
                let data = self.pop_data()?.get_interger(self)?;
                if data != 0 {
                    self.push_unresolved_op_at(op_index, &[StackItem::Pkglen, StackItem::TermArg])?;
                    self.stack.push(StackItem::Conditional {
                        ret_index: op_index + 1,
                        old_length: self.stream.length,
                    })?;
                    self.stream.length = op_index + len + 1;
                } else {
                    self.stream.index = op_index + len + 1;
                }
                Ok(())
            }

            AmlOp::ReturnOp => {
                //Have to deref in case it is a local value
                let data = self.pop_data()?;
                let data = data.resolve_locals(self)?.clone();
                serial_println!("returing : {:?}", data);
                loop {
                    match self.stack.pop()? {
                        Some(StackItem::MethodCall {
                            mut prev_stream,
                            old_method_ptr,
                            old_tlr,
                            arg_c,
                        }) => {
                            self.swp_byte_stream(&mut prev_stream);
                            self.method_ptr = old_method_ptr;
                            self.set_tlr(old_tlr);
                            //pop locals and args
                            for _ in 0..(8 + arg_c) {
                                self.stack.pop()?;
                            }
                            break;
                        }
                        Some(_) => (),
                        None => Err(AmlError::StackCorruption)?,
                    }
                }
                self.stack.push(StackItem::Data(data))?;
                self.stack.push(StackItem::Return)
            }

            AmlOp::IfOp => {
                let len = self.pop_data()?.get_interger(self)? as usize;

                let mut original_index = self.stream.index;
                self.stream.index = op_index + 1 + len;

                serial_println!(
                    "Original Index : {}, stream index : {}",
                    original_index,
                    self.stream.index
                );

                let data = self.pop_data()?.get_interger(self)?;
                if data != 0 {
                    //true

                    serial_println!("True! : {:?}", self.peek_opcode());
                    //extend if else op is after this if
                    if matches!(self.peek_opcode(), Ok(AmlOp::ElseOp)) {
                        self.next()?;
                        let start = self.stream.index;
                        let len = self.decode_pkg_length()?;
                        self.stream.index = start + len;
                    }
                    core::mem::swap(&mut original_index, &mut self.stream.index);
                    self.stack.push(StackItem::Conditional {
                        ret_index: original_index,
                        old_length: self.stream.length,
                    })?;
                    self.stream.length = op_index + 1 + len;
                } else {
                    //false
                    serial_println!("False!: {:?}", self.peek_opcode());
                    if matches!(self.peek_opcode(), Ok(AmlOp::ElseOp)) {
                        self.next()?;
                        let _ = self.decode_pkg_length()?;
                    }
                }
                Ok(())
            }

            AmlOp::DecrementOp => {
                let mut name_res = self.pop_data()?;
                let int = name_res.get_interger(self)?.wrapping_sub(1);

                name_res.mutate(AmlData::Interger(int), self)?;
                self.stack.push_data(name_res.deref(self)?.clone())
            }

            AmlOp::IncrementOp => {
                let mut name_res = self.pop_data()?;

                let int = name_res.get_interger(self)?.wrapping_add(1);
                name_res.mutate(AmlData::Interger(int), self)?;

                self.stack.push_data(name_res.deref(self)?.clone())
            }

            //Conversions
            AmlOp::ToBufferOp => {
                let data = self.pop_data()?;
                let mut target = self.pop_data()?;
                let buffer = match data {
                    AmlData::Buffer(buf) => buf,
                    AmlData::String(string) if string.is_empty() => Buffer {
                        buffer: unsafe { Box::new_zeroed_slice(0).assume_init() },
                    },
                    AmlData::String(string) => {
                        let mut str_buf = Box::new_zeroed_slice(string.len() + 1);
                        for i in 0..string.len() {
                            str_buf[i].write(string.get(i).ok_or(AmlError::IndexOutOfBounds(i))?);
                        }
                        Buffer {
                            buffer: unsafe { str_buf.assume_init() },
                        }
                    }
                    AmlData::Interger(int) => Buffer {
                        buffer: Box::new(int.to_le_bytes()),
                    },
                    data => Err(AmlError::InvalidAmlData(data, "ToBuffer resolve"))?,
                };
                target.store(self, AmlData::Buffer(buffer))
            }

            AmlOp::ToDecimalStringOp => {
                use core::fmt::Write;
                let data = self.pop_data()?;
                let mut target = self.pop_data()?;
                let buffer = match data {
                    AmlData::String(string) => string,
                    AmlData::Buffer(buf) if buf.buffer.is_empty() => AmlString::from_bytes(&[])?,
                    AmlData::Buffer(buf) => {
                        let mut buf = buf.buffer.into_iter();
                        let mut string =
                            format!("{}", buf.next().ok_or(AmlError::IndexOutOfBounds(0))?);
                        for byte in buf {
                            write!(string, ",{byte}")
                                .map_err(|_e| AmlError::InvalidData(byte, "ToDecimalStringOp"))?;
                        }
                        AmlString::from_string(string)?
                    }
                    AmlData::Interger(int) => AmlString::from_string(int.to_string())?,
                    data => Err(AmlError::InvalidAmlData(data, "ToDecimalString resolve"))?,
                };
                target.store(self, AmlData::String(buffer))
            }

            AmlOp::ToHexStringOp => {
                use core::fmt::Write;
                let data = self.pop_data()?;
                let mut target = self.pop_data()?;
                let buffer = match data {
                    AmlData::String(string) => string,
                    AmlData::Buffer(buf) if buf.buffer.is_empty() => AmlString::from_bytes(&[])?,
                    AmlData::Buffer(buf) => {
                        let mut buf = buf.buffer.into_iter();
                        let mut string =
                            format!("{:#0x}", buf.next().ok_or(AmlError::IndexOutOfBounds(0))?);
                        for byte in buf {
                            write!(string, ",{byte:#0x}")
                                .map_err(|_e| AmlError::InvalidData(byte, "ToDecimalStringOp"))?;
                        }
                        AmlString::from_string(string)?
                    }
                    AmlData::Interger(int) => AmlString::from_string(format!("{int:#0x}"))?,
                    data => Err(AmlError::InvalidAmlData(data, "ToDecimalString resolve"))?,
                };
                target.store(self, AmlData::String(buffer))
            }

            AmlOp::ToStringOp => {
                let source = self.pop_data()?.buffer(self)?;
                let length = min(
                    source.buffer.len(),
                    self.pop_data()?.get_interger(self)? as usize,
                );
                let mut target = self.pop_data()?;
                let string = AmlString::from_bytes(&source.buffer[..length])?;
                target.store(self, AmlData::String(string))
            }

            //Method invokation
            AmlOp::NameChar | AmlOp::Rootchar | AmlOp::ParentPrefixChar => {
                if let AmlData::ObjRef(method_ref) = self.pop_data()? {
                    let node = self.get_node(method_ref)?;
                    if let AmlData::Method(method) = node.data.clone() {
                        method.invoke(self, method_ref, &node.name.clone())
                    } else {
                        Err(AmlError::StackCorruption)
                    }
                } else {
                    Err(AmlError::StackCorruption)
                }
            }

            AmlOp::BufferOp => {
                let mut pkg_len = self.stack.pop_data()?.get_interger(self)?;
                let size = self.stack.pop_data()?.get_interger(self)?;
                pkg_len -= self.stream.index as u64 - op_index as u64 - 1;
                let buffer = Buffer::new(pkg_len, size, self.stream())?;
                self.stack.push_data(AmlData::Buffer(buffer))
            }

            AmlOp::ConcatOp => {
                let source_1 = self.pop_data()?;
                let source_2 = self.pop_data()?;
                let mut target = self.pop_data()?;
                let concatted = source_1.concat(source_2, self)?;
                target.store(self, concatted)
            }

            AmlOp::RefOfOp => {
                let data = self.stack.pop_data()?;
                let ptr = match data {
                    AmlData::ArgRef(stack_ref) | AmlData::LocalRef(stack_ref) => {
                        AmlData::StackRef(stack_ref)
                    }
                    r @ AmlData::ObjRef(_) => r,
                    _ => Err(AmlError::InvalidAmlData(data, "Refof"))?,
                };
                self.stack.push_data(ptr)
            }

            AmlOp::CopyObjectOp => {
                let data = self.stack.pop_data()?;
                let name_res = self.stack.pop_data()?;
                name_res.copy(self, data)
            }
            AmlOp::LnotOp => {
                let int = self.stack.pop_data()?.get_interger(self)?;
                if int != 0 {
                    self.stack.push_data(AmlData::Interger(0))
                } else {
                    self.stack.push_data(AmlData::Interger(1))
                }
            }
            AmlOp::AddOp => self.mut_interger(u64::wrapping_add),
            AmlOp::SubtractOp => self.mut_interger(u64::wrapping_sub),
            AmlOp::MultiplyOp => self.mut_interger(u64::wrapping_mul),
            AmlOp::AndOp => self.mut_interger(BitAnd::<u64>::bitand),
            AmlOp::OrOp => self.mut_interger(BitOr::<u64>::bitor),
            AmlOp::XorOp => self.mut_interger(BitXor::<u64>::bitxor),
            AmlOp::LandOp => self.cmp_interger(|a, b| *a != 0 && *b != 0),
            AmlOp::LorOp => self.cmp_interger(|a, b| *a != 0 || *b != 0),
            AmlOp::LequalOp => self.cmp_interger(u64::eq),
            AmlOp::LgreaterOp => self.cmp_interger(u64::gt),
            AmlOp::LLlessOp => self.cmp_interger(u64::lt),
            op => Err(AmlError::UnexpectedOp(op, "Resolve op")),
        }
    }

    pub fn exec_term_obj(&mut self) -> Result<(), AmlError> {
        let op = self.next_opcode()?;
        serial_println!("OP : {:?}", op);
        match op {
            AmlOp::ReturnOp => self.push_unresolved_op(&[StackItem::TermArg]),
            AmlOp::NameOp => {
                self.push_unresolved_op(&[StackItem::NameString, StackItem::DataRefObj])
            }

            AmlOp::ScopeOp => {
                let start = self.stream.index;
                let len = self.decode_pkg_length()?;
                let node = self.resolve_name_string()?;
                if node.is_null() {
                    self.stream.index = start + len;
                } else {
                    self.stack.push(StackItem::Scope {
                        old_length: self.stream.length,
                        old_tlr: self.get_tlr(),
                    })?;
                    self.stream.length = start + len;
                    self.top_level_node = node;
                }
                Ok(())
            }

            AmlOp::ContinueOp => {
                while let Some(data) = self.stack.pop_scoped()? {
                    //if the return if ret_index is higher then the current index then it is a if function
                    if let StackItem::Conditional {
                        ret_index,
                        old_length,
                    } = data
                        && ret_index < self.stream.index
                    {
                        self.stream.index = ret_index;
                        self.stream.length = old_length;
                        break;
                    }
                }
                Ok(())
            }
            AmlOp::BreakOp => {
                while let Some(data) = self.stack.pop_scoped()? {
                    //if the return if ret_index is higher then the current index then it is a if function
                    if let StackItem::Conditional {
                        ret_index,
                        old_length,
                    } = data
                        && ret_index < self.stream.index
                    {
                        //Delete old stack items
                        self.stack.pop()?;
                        self.stack.pop()?;
                        self.stack.pop()?;
                        self.stream.index = self.stream.length;
                        self.stream.length = old_length;
                        break;
                    }
                }
                Ok(())
            }

            AmlOp::MethodOp => {
                let start = self.stream.index();
                let length = self.decode_pkg_length()?;
                let end = start + length;
                let name = NameString::exec(self)?;
                let method_flags = load_byte(self)?;
                let data = self
                    .stream()
                    .substream_untill(end)
                    .ok_or(AmlError::ScopeTooLong(end))?;
                let data = AmlData::Method(Method::new(data, method_flags));
                self.push_node(name, data)?;
                self.stream().index = end;
                Ok(())
            }

            AmlOp::WhileOp | AmlOp::IfOp => {
                self.push_unresolved_op(&[StackItem::Pkglen, StackItem::TermArg])
            }

            AmlOp::ExtOpPrefix => {
                let op = self.next_ext_opcode()?;
                self.parse_ext_expr_opcode(op)
            }
            op => self.parse_expr_opcode(op),
        }
    }

    pub fn parse_ext_expr_opcode(&mut self, op: ExtOp) -> Result<(), AmlError> {
        use StackItem::{NameString, SuperName, Target, TermArg};
        match op {
            ExtOp::CondRefOfOp => self.push_unresolved_ext_op(&[SuperName, Target]),
            ExtOp::LoadOp => self.push_unresolved_ext_op(&[NameString, Target]),
            ExtOp::ToBCDOp => self.push_unresolved_ext_op(&[TermArg, Target]),
            op => Err(AmlError::UnexpectedExtOp(op, "ExprExtParse")),
        }
    }

    pub fn parse_expr_opcode(&mut self, op: AmlOp) -> Result<(), AmlError> {
        use StackItem::{SimpleName, SuperName, Target, TermArg};
        match op {
            AmlOp::ToStringOp
            | AmlOp::ConcatOp
            | AmlOp::AndOp
            | AmlOp::XorOp
            | AmlOp::OrOp
            | AmlOp::AddOp
            | AmlOp::SubtractOp
            | AmlOp::MultiplyOp => self.push_unresolved_op(&[TermArg, TermArg, Target]),
            AmlOp::LandOp
            | AmlOp::LorOp
            | AmlOp::LequalOp
            | AmlOp::LgreaterOp
            | AmlOp::LLlessOp => self.push_unresolved_op(&[TermArg, TermArg]),
            AmlOp::DerefOfOp | AmlOp::LnotOp => self.push_unresolved_op(&[TermArg]),
            AmlOp::RefOfOp | AmlOp::DecrementOp | AmlOp::IncrementOp => {
                self.push_unresolved_op(&[SuperName])
            }

            AmlOp::ToBufferOp | AmlOp::ToDecimalStringOp | AmlOp::ToHexStringOp => {
                self.push_unresolved_op(&[TermArg, Target])
            }

            AmlOp::VarPackageOp => {
                todo!("Not var package op implemented yet :3");
            }

            AmlOp::PackageOp => {
                let start = self.stream.index;
                let len = self.decode_pkg_length()?;
                let num_elements = load_byte(self)? as usize;
                self.stack.push(StackItem::Package {
                    old_length: self.stream.length,
                    package: Package::new(num_elements)?,
                    inserted: 0,
                })?;
                self.stream.length = start + len;
                Ok(())
            }

            AmlOp::StoreOp => self.push_unresolved_op(&[TermArg, SuperName]),
            AmlOp::CopyObjectOp => self.push_unresolved_op(&[TermArg, SimpleName]),

            //Metod call
            AmlOp::NameChar | AmlOp::Rootchar | AmlOp::ParentPrefixChar => {
                self.stream.index -= 1;

                let start = self.stream.index;
                let method_ref = self.resolve_name_string()?;
                let node = self.get_node(method_ref)?;

                match node.data {
                    AmlData::Method(Method { arg_c, .. }) => {
                        for _ in 0..arg_c {
                            self.stack.push(StackItem::TermArg)?;
                        }
                        self.stack
                            .push(StackItem::Data(AmlData::ObjRef(method_ref)))?;
                        self.stack.push(StackItem::UnreselvedOp {
                            op_index: start,
                            arg_c: arg_c + 1,
                            arg_s: 1,
                        })
                    }
                    _ => self.stack.push_data(node.data.clone()),
                }
            }

            op => Err(AmlError::UnexpectedOp(op, "ExprParse")),
        }
    }

    pub fn resolve_computational_data(&mut self, op: AmlOp) -> Result<(), AmlError> {
        let res = match op {
            AmlOp::ZeroOp => AmlData::Interger(0x0),
            AmlOp::OneOp => AmlData::Interger(0x1),
            AmlOp::ByteOp => AmlData::Interger(u64::from(load_byte(self)?)),
            AmlOp::WordOp => AmlData::Interger(u64::from(load_word(self)?)),
            AmlOp::DWordOp => AmlData::Interger(u64::from(load_dword(self)?)),
            AmlOp::QWordOp => AmlData::Interger(load_qword(self)?),
            AmlOp::StringOp => AmlData::String(AmlString::exec(self)?),
            AmlOp::OnesOp => AmlData::Interger(u64::MAX),
            // AmlOp::BufferOp => AmlData::Buffer(Buffer::load(self)?),
            AmlOp::ExtOpPrefix => match self.peek_ext_opcode()? {
                ExtOp::RevisionOp => {
                    self.stream.next();
                    let next = self.next()?;
                    return self.stack.push_data(AmlData::Interger(u64::from(next)));
                }
                other => Err(AmlError::UnexpectedExtOp(other, "ComputationalData"))?,
            },
            AmlOp::BufferOp => {
                return self.push_unresolved_op(&[StackItem::Pkglen, StackItem::TermArg]);
            }

            op => Err(AmlError::UnexpectedOp(op, "ComputationalData"))?,
        };
        self.stack.push_data(res)
    }

    pub fn resolve_path(&mut self, start_obj: ObjRef) -> Result<ObjRef, AmlError> {
        if start_obj.is_null() {
            return Ok(ObjRef::null());
        }

        let opcode = self.peek_opcode()?;
        match opcode {
            AmlOp::ZeroOp => {
                self.next()?;
                Ok(start_obj)
            }
            AmlOp::DualNameOp => {
                self.next()?;
                let name = NameSeg::exec(self)?;
                let child = self.get_node(start_obj).map_or(ObjRef::null(), |n| {
                    n.seek_scope(self, &name).unwrap_or(ObjRef::null())
                });

                if child.is_null() {
                    NameSeg::exec(self)?;
                    return Ok(ObjRef::null());
                }
                let name = NameSeg::exec(self)?;

                self.get_node(child)?.find_child(self, &name)
            }
            AmlOp::MultiNameOp => {
                self.next()?;
                let seg_count = self.next()?;
                let first_name = NameSeg::exec(self)?;

                let mut node = self.get_node(start_obj).map_or(ObjRef::null(), |n| {
                    n.seek_scope(self, &first_name).unwrap_or(ObjRef::null())
                });
                for _ in 1..seg_count {
                    let name = NameSeg::exec(self)?;
                    if !node.is_null() {
                        node = self.get_node(node)?.find_child(self, &name)?;
                    }
                }
                Ok(node)
            }
            AmlOp::NameChar => {
                let name = NameSeg::exec(self)?;

                Ok(self.get_tln()?.seek_scope(self, &name)?)
            }
            op => Err(AmlError::UnexpectedOp(op, "NamePath"))?,
        }
    }

    ///Resolves the name path and returns the refrence back.
    pub fn resolve_name_string(&mut self) -> Result<ObjRef, AmlError> {
        let mut depth = 0;
        loop {
            match self.peek_opcode()? {
                AmlOp::ParentPrefixChar => {
                    depth += 1;
                    self.next()?;
                }
                AmlOp::Rootchar => {
                    self.next()?;
                    let mut node = self.get_tlr();

                    while let Some(parent) = self.get_node(node)?.parent {
                        node = parent;
                    }
                    return self.resolve_path(node);
                }
                AmlOp::DualNameOp | AmlOp::MultiNameOp | AmlOp::NameChar => break,
                op => Err(AmlError::UnexpectedOp(op, "Resolve name string"))?,
            }
        }

        let mut node = self.get_tlr();
        for _ in 0..depth {
            if let Some(obj_ref) = self.get_node(node)?.parent {
                node = obj_ref;
            } else {
                break;
            }
        }

        self.resolve_path(node)
    }

    pub fn resolve_simple_name(&mut self) -> Result<(), AmlError> {
        let opcode = self.peek_opcode()?;
        let res = match opcode {
            AmlOp::Arg0
            | AmlOp::Arg1
            | AmlOp::Arg2
            | AmlOp::Arg3
            | AmlOp::Arg4
            | AmlOp::Arg5
            | AmlOp::Arg6 => {
                let arg = ArgObj::exec(self)?;
                let arg_ref = self.get_arg_ref(arg)?;
                AmlData::ArgRef(arg_ref)
            }
            AmlOp::Local0
            | AmlOp::Local1
            | AmlOp::Local2
            | AmlOp::Local3
            | AmlOp::Local4
            | AmlOp::Local5
            | AmlOp::Local6
            | AmlOp::Local7 => {
                let local = LocalObj::exec(self)?;
                let local_ref = self.get_local_ref(local)?;
                AmlData::LocalRef(local_ref)
            }
            _ => {
                let obj_ref = self.resolve_name_string()?;
                return self.stack.push_data(AmlData::ObjRef(obj_ref));
            }
        };

        self.stack.push_data(res)
    }

    pub fn resolve_super_name(&mut self) -> Result<(), AmlError> {
        let op = self.peek_opcode()?;
        let res = match op {
            AmlOp::ExtOpPrefix => {
                self.next()?;
                let op = self.next_ext_opcode()?;
                if op == ExtOp::DebugOp {
                    AmlData::Debug
                } else {
                    Err(AmlError::UnexpectedExtOp(op, "SuperName"))?
                }
            }
            AmlOp::RefOfOp => {
                todo!("Ref op of super name is bad");
                let raw_ref = self.pop_data()?.get_interger(self)?;
                AmlData::ObjRef(ObjRef::from_raw_usize(raw_ref as usize).ok_or(AmlError::NullRef)?)
            }
            _ => return self.resolve_simple_name(),
        };
        self.stack.push_data(res)
    }

    pub fn resolve_target(&mut self) -> Result<(), AmlError> {
        let op = self.peek_opcode()?;
        let res = match op {
            AmlOp::ZeroOp => {
                self.next()?;
                AmlData::Null
            }
            _ => return self.resolve_super_name(),
        };
        self.stack.push_data(res)
    }

    fn mut_interger<F: FnOnce(u64, u64) -> u64>(&mut self, f: F) -> Result<(), AmlError> {
        let rhs = self.pop_data()?.get_interger(self)?;
        let lhs = self.pop_data()?.get_interger(self)?;
        let mut name_res = self.pop_data()?;
        name_res.store(self, AmlData::Interger(f(rhs, lhs)))
    }

    fn cmp_interger<F: FnOnce(&u64, &u64) -> bool>(&mut self, f: F) -> Result<(), AmlError> {
        let lhs = self.pop_data()?.get_interger(self)?;
        let rhs = self.pop_data()?.get_interger(self)?;
        if f(&lhs, &rhs) {
            self.stack.push_data(AmlData::Interger(1))
        } else {
            self.stack.push_data(AmlData::Interger(0))
        }
    }

    fn push_node(&mut self, name: NameString, data: AmlData) -> Result<(), AmlError> {
        let (path, node) = match name {
            NameString::Rel(path, parents) => {
                let mut node = self.get_tlr();
                for _ in 0..parents {
                    if let Some(parent_ref) = self.get_node(node)?.parent {
                        node = parent_ref;
                    } else {
                        break;
                    }
                }
                (path, node)
            }
            NameString::Root(path) => {
                let mut node = self.get_tlr();
                while let Some(parent_ref) = self.get_node(node)?.parent {
                    node = parent_ref;
                }
                (path, node)
            }
        };

        let (name, node) = match &path {
            NamePath::NullName => Err(AmlError::NullObj)?,
            NamePath::NameSeg(name) => (name, node),
            NamePath::DualNamePath(in_name, name) => {
                let node = self.get_node(node)?.find_child(self, in_name)?;
                (name, node)
            }
            NamePath::MultiNamePath(names) => {
                let mut node = node;
                let (name, path) = names
                    .split_last()
                    .ok_or(AmlError::NamedObjectNotFound(path.clone()))?;
                for name in path {
                    node = self.get_node(node)?.find_child(self, name)?;
                    if node.is_null() {
                        break;
                    }
                }
                (name, node)
            }
        };

        if node.is_null() {
            Ok(())
        } else {
            self.append_to_node(node, NameNode::new(name.clone(), data))?;
            Ok(())
        }
    }

    /// Appends a child to a parent node making sure that the parent gets set correctly
    fn append_to_node(&mut self, parent: ObjRef, mut child: NameNode) -> Result<ObjRef, AmlError> {
        child.parent = Some(parent);
        {
            let parent = self
                .heap
                .get(parent)
                .ok_or(AmlError::InvalidObjectRef(parent))?;
            if parent.has_child(self, &child.name)? {
                Err(AmlError::DuplicateName(child.name.clone()))?;
            }
        }

        let child_ref = self.heap.instert(child);

        let parent = self
            .heap
            .get_mut(parent)
            .ok_or(AmlError::InvalidObjectRef(parent))?;

        parent.append_child(child_ref);
        Ok(child_ref)
    }

    pub fn fn_for<D, F: Fn(&mut Self) -> Result<D, AmlError>>(
        &mut self,
        count: usize,
        length: usize,
        f: F,
    ) -> Result<Box<[D]>, AmlError> {
        let start = self.stream().index();
        let mut stream = self
            .stream()
            .substream_untill(start + length)
            .ok_or(AmlError::ScopeTooLong(length))?;
        core::mem::swap(&mut self.stream, &mut stream);

        let mut res = vec![];
        res.reserve_exact(count);

        for _ in 0..length {
            res.push(f(self)?);
        }

        self.stream = stream;
        self.stream.index += 1 + length;
        Ok(res.into_boxed_slice())
    }

    pub fn exec_package_element(&mut self) -> Result<AmlData, AmlError> {
        Ok(AmlData::NameString(NameString::exec(self)?))
    }

    pub fn exec_term_arg(&mut self) -> Result<(), AmlError> {
        let op = self.next_opcode()?;

        if matches!(op, AmlOp::ExtOpPrefix) {
            let op = self.next_ext_opcode()?;
            match self.parse_ext_expr_opcode(op) {
                Ok(()) => return Ok(()),
                Err(AmlError::UnexpectedOp(_, _) | AmlError::UnexpectedExtOp(_, _)) => (),
                e => return e,
            }
        } else {
            match self.resolve_computational_data(op) {
                Ok(()) => return Ok(()),
                Err(AmlError::UnexpectedOp(_, _) | AmlError::UnexpectedExtOp(_, _)) => (),
                e => return e,
            }

            match self.parse_expr_opcode(op) {
                Ok(()) => return Ok(()),
                Err(AmlError::UnexpectedOp(_, _) | AmlError::UnexpectedExtOp(_, _)) => (),
                e => return e,
            }

            match self.parse_data_object(op) {
                Ok(()) => return Ok(()),
                Err(AmlError::UnexpectedOp(_, _) | AmlError::UnexpectedExtOp(_, _)) => (),
                e => return e,
            }

            match ArgObj::parse(op) {
                Ok(arg) => {
                    let arg_ref = self.get_arg_ref(arg)?;
                    let data = self.get_stack(arg_ref)?;
                    let dataderef = data.deref(self)?;
                    self.stack.push_data(dataderef.clone())?;
                    return Ok(());
                }
                Err(AmlError::InvalidData(_, _)) => (),
                Err(e) => Err(e)?,
            }

            match LocalObj::parse(op) {
                Ok(local) => {
                    let local_ref = self.get_local_ref(local)?;
                    let data = self.get_stack(local_ref)?;
                    self.stack.push_data(data.clone());
                    return Ok(());
                }
                Err(AmlError::InvalidData(_, _)) => (),
                Err(e) => Err(e)?,
            }
        }

        Err(AmlError::UnexpectedOp(op, "TermArg"))
    }

    pub fn resolve_computational_data_ext(&mut self, op: ExtOp) -> Result<(), AmlError> {
        let res = match op {
            ExtOp::ReleaseOp => AmlData::Interger(u64::from(load_byte(self)?)),
            op => Err(AmlError::UnexpectedExtOp(op, "ComputationalData"))?,
        };
        self.stack.push_data(res)
    }

    #[inline]
    pub fn parse_data_object(&mut self, op: AmlOp) -> Result<(), AmlError> {
        match op {
            AmlOp::PackageOp => self.push_unresolved_op(&[StackItem::Pkglen, StackItem::Byte]),
            AmlOp::VarPackageOp => {
                self.push_unresolved_op(&[StackItem::Pkglen, StackItem::TermArg])
            }
            _ => self.resolve_computational_data(op),
        }
    }

    #[inline]
    pub fn parse_data_ref_object(&mut self, op: AmlOp) -> Result<(), AmlError> {
        self.parse_data_object(op)
    }

    ///Finds a node from a namepath
    pub fn find_node_path(&self, mut node: ObjRef, path: NamePath) -> Result<ObjRef, AmlError> {
        for seg in path.iter() {
            node = self.get_node(node)?.find_child(self, seg)?;
        }
        Ok(node)
    }

    pub fn find_node_string(&self, name: NameString) -> Result<ObjRef, AmlError> {
        match name {
            NameString::Rel(path, rel) => {
                let mut root = self.get_tlr();
                for _ in 0..rel {
                    let node = self.get_node(root)?;
                    if let Some(parent) = node.parent {
                        root = parent;
                    } else {
                        Err(AmlError::NamedObjectNotFound(path.clone()))?
                    }
                }
                self.find_node_path(root, path)
            }
            NameString::Root(path) => {
                let mut root = self.get_tlr();
                while let Ok(node) = self.get_node(root)
                    && let Some(parent) = node.parent
                {
                    root = parent;
                }
                self.find_node_path(root, path)
            }
        }
    }

    pub fn get_or_exec(
        &mut self,
        path: NamePath,
        call_args: &[AmlData],
    ) -> Result<AmlData, AmlError> {
        let node = self.find_node_path(self.get_tlr(), path)?;
        let (name, data) = {
            let node = self.get_node(node)?;
            let name = node.name.clone();
            let data = node.data.clone();
            (name, data)
        };

        if let AmlData::Method(method) = data {
            if usize::from(method.arg_c) != call_args.len() {
                Err(AmlError::InvalidArgCount(call_args.len(), method.arg_c))?;
            }

            method.call(node, self, call_args, &name)
        } else {
            Ok(data)
        }
    }

    pub fn decode_pkg_length(&mut self) -> Result<usize, AmlError> {
        let pkg_lead_byte = self.next()?;
        let bytedata_count = (pkg_lead_byte >> 6) & 0b11;
        Ok(match bytedata_count {
            0b00 => u32::from(pkg_lead_byte & 0b0011_1111),
            0b01 => u32::from(pkg_lead_byte & 0b0000_1111) | u32::from(self.next()?) << 4,
            0b10 => {
                u32::from(pkg_lead_byte & 0b0000_1111)
                    | u32::from(self.next()?) << 4
                    | u32::from(self.next()?) << 12
            }
            0b11 => {
                u32::from(pkg_lead_byte & 0b0000_1111)
                    | u32::from(self.next()?) << 4
                    | u32::from(self.next()?) << 12
                    | u32::from(self.next()?) << 20
            }
            data => Err(AmlError::InvalidData(data, "Pkg length"))?,
        } as usize)
    }

    const fn pkg_len_bytes(pkg_len: usize) -> usize {
        match pkg_len {
            0..=0b11_1111 => 1,
            0x40..=0xF_FF => 2,
            0x10_00..=0xF_FF_FF => 3,
            0x10_00_00.. => 4,
        }
    }

    #[inline]
    pub const fn swp_byte_stream(&mut self, stream: &mut ByteStream) {
        core::mem::swap(&mut self.stream, stream);
    }

    #[inline]
    #[must_use]
    pub const fn stream(&mut self) -> &mut ByteStream {
        &mut self.stream
    }
    #[inline]
    #[must_use]
    pub fn ext_op_at(&self, index: usize) -> Result<ExtOp, AmlError> {
        ExtOp::try_from(self.stream.at(index).ok_or(AmlError::OutOfBoundsExec)?)
    }
    #[inline]
    #[must_use]
    pub fn op_at(&self, index: usize) -> Result<AmlOp, AmlError> {
        AmlOp::try_from(self.stream.at(index).ok_or(AmlError::OutOfBoundsExec)?)
    }
    #[inline]
    #[must_use]
    pub fn peek(&mut self) -> Result<u8, AmlError> {
        self.stream().peek().ok_or(AmlError::EndOfStream)
    }
    #[inline]
    #[must_use]
    pub fn peek_opcode(&mut self) -> Result<AmlOp, AmlError> {
        AmlOp::try_from(self.stream().peek().ok_or(AmlError::EndOfStream)?)
    }
    #[inline]
    #[must_use]
    pub fn peek_ext_opcode(&mut self) -> Result<ExtOp, AmlError> {
        ExtOp::try_from(self.stream().peek().ok_or(AmlError::EndOfStream)?)
    }

    ///Auto converts the next byte into a bytecode
    #[inline]
    #[must_use]
    pub fn next_opcode(&mut self) -> Result<AmlOp, AmlError> {
        AmlOp::try_from(self.stream().next().ok_or(AmlError::EndOfStream)?)
    }
    #[inline]
    #[must_use]
    pub fn next_ext_opcode(&mut self) -> Result<ExtOp, AmlError> {
        ExtOp::try_from(self.stream().next().ok_or(AmlError::EndOfStream)?)
    }
    #[inline]
    pub fn next(&mut self) -> Result<u8, AmlError> {
        self.stream().next().ok_or(AmlError::EndOfStream)
    }

    #[inline]
    pub fn get_node(&self, obj_ref: ObjRef) -> Result<&NameNode, AmlError> {
        self.heap
            .get(obj_ref)
            .ok_or(AmlError::InvalidObjectRef(obj_ref))
    }
    #[inline]
    pub fn mut_node(&mut self, obj_ref: ObjRef) -> Result<&mut NameNode, AmlError> {
        self.heap
            .get_mut(obj_ref)
            .ok_or(AmlError::InvalidObjectRef(obj_ref))
    }

    pub fn get_stack(&self, index: usize) -> Result<&AmlData, AmlError> {
        self.stack
            .peek_at(index)
            .map(|stack_item| match stack_item {
                StackItem::Data(data) => Ok(data),
                _ => Err(AmlError::StackCorruption),
            })
            .ok_or(AmlError::StackCorruption)?
    }

    pub fn get_stack_mut(&mut self, index: usize) -> Result<&mut AmlData, AmlError> {
        self.stack
            .peek_at_mut(index)
            .map(|stack_item| match stack_item {
                StackItem::Data(data) => Ok(data),
                _ => Err(AmlError::StackCorruption),
            })
            .ok_or(AmlError::StackCorruption)?
    }

    ///Returns a stack pointer to the given local
    pub const fn get_local_ref(&self, local: LocalObj) -> Result<usize, AmlError> {
        if self.method_ptr == 0 {
            Err(AmlError::NotInMethod)
        } else {
            Ok(self.method_ptr - local.0 as usize)
        }
    }

    ///Returns a stack pointer to the given arg
    pub fn get_arg_ref(&self, arg: ArgObj) -> Result<usize, AmlError> {
        if self.method_ptr == 0 {
            Err(AmlError::NotInMethod)
        } else {
            match self.stack.peek_at(self.method_ptr) {
                Some(StackItem::MethodCall { arg_c, .. }) if arg.0 > *arg_c => {
                    Err(AmlError::InvalidArgIndex(arg.0))?;
                }
                _ => (),
            }

            Ok(self.method_ptr - LOCAL_COUNT - arg.0 as usize)
        }
    }

    /// Gets a obj refrence to current top level node from the context
    #[must_use]
    pub const fn get_tlr(&self) -> ObjRef {
        self.top_level_node
    }

    pub const fn set_tlr(&mut self, obj_ref: ObjRef) {
        self.top_level_node = obj_ref
    }

    ///Gets the current top level node
    pub fn get_tln(&self) -> Result<&NameNode, AmlError> {
        self.get_node(self.get_tlr())
    }

    ///Gets a mutable refrence to the top level node
    pub fn get_tln_mut(&mut self) -> Result<&mut NameNode, AmlError> {
        self.mut_node(self.get_tlr())
    }

    fn push_unresolved_ext_op(&mut self, args: &[StackItem]) -> Result<(), AmlError> {
        self.stack.ext_op_arg_push(self.stream.index - 1, args)
    }

    fn push_unresolved_op_at(&mut self, index: usize, args: &[StackItem]) -> Result<(), AmlError> {
        self.stack.op_arg_push(index, args)
    }

    ///Pushes a unresolved op to the stack
    fn push_unresolved_op(&mut self, args: &[StackItem]) -> Result<(), AmlError> {
        serial_println!("Args : {:?}", args);
        // -1 due to the stream index always sitting at the next item to be read.
        self.stack.op_arg_push(self.stream.index - 1, args)
    }

    pub fn pop_data(&mut self) -> Result<AmlData, AmlError> {
        self.stack.pop_data()
    }
}
