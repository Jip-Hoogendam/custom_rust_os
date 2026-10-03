use alloc::{boxed::Box, string::String, vec};
use core::{
    error::Error,
    fmt::{Display, write},
    ops::{BitAnd, BitOr, BitXor},
    time::Duration,
};

use super::stack_items::StackItem;

use crate::{
    acpi::aml::{
        AmlError, ByteCodeInstruction, ByteCodeObject,
        data_objects::{AmlString, DataObj, Package, load_byte, load_dword, load_qword, load_word},
        named_objects::{NamePath, NameSeg, NameString, RegionSpace},
        namenode::{AmlData, Method, NameNode},
        opcodes::{AmlOp, AnyAmlOp, ExtOp},
    },
    arch::registers::sp,
    collections::typed_arena::{TypedArena, TypedArenaAllocation},
    println, serial_print, serial_println,
    time::Instant,
};

#[derive(Clone, Debug)]
pub struct ByteStream {
    bytes: *const u8,
    index: usize,
    length: usize,
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
        serial_println!("{},{},{}", end, self.index, self.length);
        if end <= self.length {
            let mut clone = self.clone();
            clone.length = core::cmp::min(end, self.length);
            Some(clone)
        } else {
            None
        }
    }

    pub const fn peek(&mut self) -> Option<u8> {
        if self.index < self.length {
            Some(unsafe { *self.bytes.add(self.index) })
        } else {
            None
        }
    }
}

impl Iterator for ByteStream {
    type Item = u8;

    ///Retruns the next byte in the stream
    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.length {
            let byte = unsafe { *self.bytes.add(self.index) };
            self.index += 1;
            Some(byte)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone, Copy)]
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
pub enum NameRes {
    Ref(ObjRef),
    Arg(ArgObj),
    Local(LocalObj),
    Index(ObjRef, usize),
    Null,
    Debug,
}

#[derive(Debug)]
pub struct AmlStackFrame {
    pub error: AmlError,
    pub stream: ByteStream,
}

impl Error for AmlStackFrame {}

impl Display for AmlStackFrame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "[AML EXEC ERROR]\nStream : {}\nbacktrace:\n{}",
            self.stream, self.error
        )
    }
}

///Aml parser and exectuor
pub struct AmlVM {
    stream: ByteStream,
    top_level_node: ObjRef,
    stack_ptr: usize,
    stack: Box<[StackItem]>,
    heap: TypedArena<NameNode>,
    args: [AmlData; 7],
    local: [AmlData; 8],
}

impl AmlVM {
    #[must_use]
    pub fn new(start_point: *const u8, length: usize, stack_size: usize) -> Self {
        let mut heap = TypedArena::new();
        let root_obj = NameNode::new(NameSeg::root(), AmlData::Null);
        let top_level_node = heap.instert(root_obj);
        Self {
            stream: ByteStream::new(start_point, length),
            heap,
            top_level_node,
            stack_ptr: 0,
            stack: vec![const { StackItem::Null }; stack_size].into_boxed_slice(),

            args: [
                AmlData::default(),
                AmlData::default(),
                AmlData::default(),
                AmlData::default(),
                AmlData::default(),
                AmlData::default(),
                AmlData::default(),
            ],

            local: [
                AmlData::default(),
                AmlData::default(),
                AmlData::default(),
                AmlData::default(),
                AmlData::default(),
                AmlData::default(),
                AmlData::default(),
                AmlData::default(),
            ],
        }
    }

    pub fn push_stack_item(&mut self, stack_item: StackItem) -> Result<(), AmlError> {
        *self
            .stack
            .get_mut(self.stack_ptr)
            .ok_or(AmlError::StackOverflow)? = stack_item;
        Ok(())
    }

    #[inline]
    pub const fn swp_args(&mut self, new_args: &mut [AmlData; 7]) {
        core::mem::swap(&mut self.args, new_args);
    }

    #[inline]
    pub fn swp_locals(&mut self, locals: &mut [AmlData; 8]) {
        core::mem::swap(&mut self.local, locals);
    }

    #[inline]
    pub fn swp_byte_stream(&mut self, stream: &mut ByteStream) {
        core::mem::swap(&mut self.stream, stream);
    }

    pub fn exec(&mut self) -> Result<(), AmlStackFrame> {
        loop {
            serial_println!("init_sp {}", sp());
            match self.exec_term_obj() {
                Ok(data) => (),
                Err(AmlError::TermEnd) => return Ok(()),
                Err(err) => {
                    return Err(AmlStackFrame {
                        error: err,
                        stream: self.stream.clone(),
                    });
                }
            }
        }
    }

    #[inline]
    #[must_use]
    pub const fn stream(&mut self) -> &mut ByteStream {
        &mut self.stream
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

    fn get_local(&mut self, local: LocalObj) -> Result<&mut AmlData, AmlError> {
        Ok(self
            .local
            .get_mut(local.0 as usize)
            .ok_or(AmlError::InvalidLocalIndex(local.0))?)
    }

    fn get_arg(&mut self, arg: ArgObj) -> Result<&mut AmlData, AmlError> {
        Ok(self
            .args
            .get_mut(arg.0 as usize)
            .ok_or(AmlError::InvalidLocalIndex(arg.0))?)
    }

    /// Gets a obj refrence to current top level node from the context
    pub fn get_tlr(&self) -> ObjRef {
        self.top_level_node
    }

    ///Gets the current top level node
    pub fn get_tln(&self) -> Result<&NameNode, AmlError> {
        self.get_node(self.get_tlr())
    }

    ///Gets a mutable refrence to the top level node
    pub fn get_tln_mut(&mut self) -> Result<&mut NameNode, AmlError> {
        self.mut_node(self.get_tlr())
    }

    pub fn resolve_path(&mut self) -> Result<Option<ObjRef>, AmlError> {
        let opcode = self.peek_opcode()?;
        match opcode {
            AmlOp::ZeroOp => {
                self.next()?;
                Ok(None)
            }
            AmlOp::DualNameOp => {
                self.next()?;
                let name = NameSeg::exec(self)?;
                let child = self.get_tln()?.seek_scope(self, &name)?;
                let name = NameSeg::exec(self)?;
                let child = self.get_node(child)?.find_child(self, &name)?;
                Ok(Some(child))
            }
            AmlOp::MultiNameOp => {
                self.next();
                let seg_count = self.next()?;
                let first_name = NameSeg::exec(self)?;
                let mut node = self.get_tln()?.seek_scope(self, &first_name)?;
                for _ in 1..seg_count {
                    let name = NameSeg::exec(self)?;
                    node = self.get_node(node)?.find_child(self, &name)?;
                }
                Ok(Some(node))
            }
            b if matches!(b as u8, b'A'..=b'Z' | b'_') => {
                let name = NameSeg::exec(self)?;
                Ok(Some(self.get_tln()?.seek_scope(self, &name)?))
            }
            op => Err(AmlError::UnexpectedOp(op, "NamePath")),
        }
    }

    pub fn resolve_simple_name(&mut self) -> Result<NameRes, AmlError> {
        let opcode = self.peek_opcode()?;
        match opcode {
            AmlOp::Arg0
            | AmlOp::Arg1
            | AmlOp::Arg2
            | AmlOp::Arg3
            | AmlOp::Arg4
            | AmlOp::Arg5
            | AmlOp::Arg6 => {
                let arg = ArgObj::exec(self)?;
                Ok(NameRes::Arg(arg))
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
                Ok(NameRes::Local(local))
            }
            _ => match self.resolve_path()? {
                Some(obj_ref) => Ok(NameRes::Ref(obj_ref)),
                None => Ok(NameRes::Null),
            },
        }
    }

    pub fn resolve_super_name(&mut self) -> Result<NameRes, AmlError> {
        let op = self.peek_opcode()?;
        match op {
            AmlOp::ExtOpPrefix => {
                self.next()?;
                let op = self.next_ext_opcode()?;
                if op == ExtOp::DebugOp {
                    Ok(NameRes::Debug)
                } else {
                    Err(AmlError::UnexpectedExtOp(op, "SuperName"))
                }
            }
            AmlOp::RefOfOp => {
                let raw_ref = self.exec_term_arg()?.interger()?;
                Ok(NameRes::Ref(
                    ObjRef::from_raw_usize(raw_ref as usize).ok_or(AmlError::NullRef)?,
                ))
            }
            op => match self.resolve_simple_name() {
                Ok(d) => Ok(d),
                Err(AmlError::UnexpectedOp(op, _)) => Err(AmlError::UnexpectedOp(op, "SuperName")),
                Err(AmlError::UnexpectedExtOp(op, _)) => {
                    Err(AmlError::UnexpectedExtOp(op, "SuperName"))
                }
                e => e,
            },
        }
    }

    pub fn resolve_target(&mut self) -> Result<NameRes, AmlError> {
        let op = self.peek_opcode()?;
        match op {
            AmlOp::ZeroOp => Ok(NameRes::Null),
            _ => self.resolve_super_name(),
        }
    }

    fn mutate_node<F: FnOnce(&mut AmlData, AmlData) -> Result<AmlData, AmlError>>(
        &mut self,
        name_res: NameRes,
        f: F,
        input: AmlData,
    ) -> Result<AmlData, AmlError> {
        let data = match name_res {
            NameRes::Ref(obj_ref) => &mut self.mut_node(obj_ref)?.data,
            NameRes::Arg(arg) => self.get_arg(arg)?,
            NameRes::Local(local) => self.get_local(local)?,
            NameRes::Debug => Err(AmlError::MutDebugObj)?,
            NameRes::Index(obj_ref, index) => {
                todo!();
            }
            NameRes::Null => &mut self.get_tln_mut()?.data,
        };
        *data = f(data, input)?;
        Ok(data.clone())
    }

    fn set_node(&mut self, name_res: NameRes, data: AmlData) -> Result<(), AmlError> {
        match name_res {
            NameRes::Ref(obj_ref) => self.mut_node(obj_ref)?.data = data,
            NameRes::Arg(arg) => *self.get_arg(arg)? = data,
            NameRes::Local(local) => *self.get_local(local)? = data,
            NameRes::Debug => println!("AmlDebug: {:?}", data),
            NameRes::Index(obj_ref, index) => {
                todo!();
            }
            NameRes::Null => self.get_tln_mut()?.data = data,
        }
        Ok(())
    }

    fn push_node(&mut self, name: NameString, data: AmlData) -> Result<ObjRef, AmlError> {
        // if no top level node set it to this node instead
        if let NameString::Rel(NamePath::NameSeg(name), 0) = name {
            let obj_ref = self.heap.instert(NameNode::new(name, data));
            let root = self.get_tln_mut()?;
            root.append_child(obj_ref);
            return Ok(obj_ref);
        }

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
                }
                (name, node)
            }
        };

        self.append_to_node(node, NameNode::new(name.clone(), data))
    }

    /// Appends a child to a parent node making sure that the parent gets set correctly
    fn append_to_node(&mut self, parent: ObjRef, mut child: NameNode) -> Result<ObjRef, AmlError> {
        child.parent = Some(parent);

        {
            let parent = self
                .heap
                .get(parent)
                .ok_or(AmlError::InvalidObjectRef(parent))?;
            if parent.find_child(self, &child.name).is_ok() {
                Err(AmlError::DuplicateName(child.name.clone()))?
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
            res.push(f(self)?)
        }

        self.stream = stream;
        self.stream.index += 1 + length;
        Ok(res.into_boxed_slice())
    }

    ///Executes data within the given scope
    pub fn exec_within(&mut self, obj_ref: ObjRef, end: usize) -> Result<(), AmlError> {
        let mut stream = self
            .stream()
            .substream_untill(end)
            .ok_or(AmlError::ScopeTooLong(end))?;

        //save values
        core::mem::swap(&mut self.stream, &mut stream);

        let curr_tln = self.top_level_node;
        self.top_level_node = obj_ref;

        if let Err(err) = self.exec() {
            let tld = self.get_node(obj_ref)?;
            return Err(AmlError::ScopeErr(tld.name.clone(), Box::new(err)));
        }

        //restore old values
        self.stream = stream;
        self.stream.index = end;
        self.top_level_node = curr_tln;
        Ok(())
    }

    pub fn exec_package_element(&mut self) -> Result<AmlData, AmlError> {
        let op = self.peek_opcode()?;
        match op {
            _ => Ok(AmlData::NameString(NameString::exec(self)?)),
        }
    }

    pub fn exec_term_arg(&mut self) -> Result<AmlData, AmlError> {
        let op = self.next_opcode()?;
        match self.parse_computational_data(op) {
            Ok(data) => return Ok(data),
            Err(AmlError::UnexpectedOp(_, _) | AmlError::UnexpectedExtOp(_, _)) => (),
            e => return e,
        }

        let sp = sp();
        match self.parse_expr_opcode(op) {
            Ok(data) => return Ok(data),
            Err(AmlError::UnexpectedOp(_, _) | AmlError::UnexpectedExtOp(_, _)) => (),
            e => return e,
        }

        match self.parse_data_object(op) {
            Ok(data) => return Ok(data),
            Err(AmlError::UnexpectedOp(_, _) | AmlError::UnexpectedExtOp(_, _)) => (),
            e => return e,
        }

        match ArgObj::parse(op) {
            Ok(arg) => return Ok(self.get_arg(arg)?.clone()),
            Err(AmlError::InvalidData(_, _)) => (),
            Err(e) => Err(e)?,
        }

        match LocalObj::parse(op) {
            Ok(arg) => return Ok(self.get_local(arg)?.clone()),
            Err(AmlError::InvalidData(_, _)) => (),
            Err(e) => Err(e)?,
        }

        Err(AmlError::UnexpectedOp(op, "TermArg"))
    }

    #[inline]
    pub fn exec_int_op<F: FnOnce(u64, u64) -> u64>(&mut self, f: F) -> Result<AmlData, AmlError> {
        let res = self.exec_lint_op(f)?;
        let target = self.resolve_target()?;
        self.set_node(target, res)?;
        Ok(AmlData::Null)
    }

    #[inline]
    pub fn exec_lint_op<F: FnOnce(u64, u64) -> u64>(&mut self, f: F) -> Result<AmlData, AmlError> {
        let lhs = self.exec_term_arg()?.interger()?;
        let rhs = self.exec_term_arg()?.interger()?;
        Ok(AmlData::Interger(f(lhs, rhs)))
    }

    pub fn conditional_op<F: FnOnce(AmlData, AmlData) -> bool>(
        &mut self,
        f: F,
    ) -> Result<AmlData, AmlError> {
        let lhs = self.exec_term_arg()?;
        let rhs = self.exec_term_arg()?;
        if f(lhs, rhs) {
            Ok(AmlData::Interger(1))
        } else {
            Ok(AmlData::Interger(0))
        }
    }

    pub fn parse_expr_opcode_ext(&mut self, op: ExtOp) -> Result<AmlData, AmlError> {
        match op {
            op => Err(AmlError::UnexpectedExtOp(op, "ExprParse Ext")),
        }
    }

    pub fn parse_expr_opcode(&mut self, op: AmlOp) -> Result<AmlData, AmlError> {
        match op {
            AmlOp::AddOp => self.exec_int_op(u64::wrapping_add),
            AmlOp::SubtractOp => self.exec_int_op(u64::wrapping_sub),
            AmlOp::DivideOp => {
                let lhs = self.exec_term_arg()?.interger()?;
                let rhs = self.exec_term_arg()?.interger()?;
                let target = self.resolve_target()?;
                if rhs == 0 {
                    Err(AmlError::DivByZero)?
                }
                self.set_node(target, AmlData::Interger(lhs.div_euclid(rhs)))?;
                todo!();
                Ok(AmlData::Null)
            }
            AmlOp::MultiplyOp => self.exec_int_op(u64::wrapping_mul),
            AmlOp::AndOp => self.exec_int_op(BitAnd::<u64>::bitand),
            AmlOp::OrOp => self.exec_int_op(BitOr::<u64>::bitor),
            AmlOp::XorOp => self.exec_int_op(BitXor::<u64>::bitxor),
            AmlOp::LandOp => self.exec_lint_op(BitAnd::<u64>::bitand),
            AmlOp::LorOp => self.exec_lint_op(BitOr::<u64>::bitor),
            AmlOp::StoreOp => {
                let data = self.exec_term_arg()?;
                let name_res = self.resolve_super_name()?;
                self.set_node(name_res, data)?;
                Ok(AmlData::Null)
            }
            AmlOp::IncrementOp => {
                let name_res = self.resolve_super_name()?;
                self.mutate_node(
                    name_res,
                    |data, input| {
                        Ok(AmlData::Interger(
                            data.interger()?.wrapping_add(input.interger()?),
                        ))
                    },
                    AmlData::Interger(1),
                )
            }
            AmlOp::DecrementOp => {
                let name_res = self.resolve_super_name()?;
                self.mutate_node(
                    name_res,
                    |data, input| {
                        Ok(AmlData::Interger(
                            data.interger()?.wrapping_sub(input.interger()?),
                        ))
                    },
                    AmlData::Interger(1),
                )
            }
            AmlOp::ExtOpPrefix => {
                let op = self.next_ext_opcode()?;
                self.parse_expr_opcode_ext(op)
            }
            //Invoke method
            AmlOp::NameChar => {
                self.stream.index -= 1; //Only place really i should be doing this
                let (data, name) = if let Some(node) = self.resolve_path()? {
                    let node = self.get_node(node)?;
                    (node.data.clone(), node.name.clone())
                } else {
                    let node = self.get_tln()?;
                    (node.data.clone(), node.name.clone())
                };

                data.invoke(self, &name)
            }
            op => Err(AmlError::UnexpectedOp(op, "ExprParse")),
        }
    }

    pub fn parse_computational_data(&mut self, op: AmlOp) -> Result<AmlData, AmlError> {
        match op {
            AmlOp::ZeroOp => Ok(AmlData::Interger(0x0)),
            AmlOp::OneOp => Ok(AmlData::Interger(0x1)),
            AmlOp::ByteOp => Ok(AmlData::Interger(u64::from(load_byte(self)?))),
            AmlOp::WordOp => Ok(AmlData::Interger(u64::from(load_word(self)?))),
            AmlOp::DWordOp => Ok(AmlData::Interger(u64::from(load_dword(self)?))),
            AmlOp::QWordOp => Ok(AmlData::Interger(load_qword(self)?)),
            AmlOp::StringOp => Ok(AmlData::String(AmlString::load(self)?)),
            AmlOp::OnesOp => Ok(AmlData::Interger(0xFF)),
            AmlOp::BufferOp => Ok(AmlData::Buffer(Buffer::load(self)?)),
            AmlOp::ExtOpPrefix => {
                let op = self.next_ext_opcode()?;
                self.parse_computational_data_ext(op)
            }

            op => Err(AmlError::UnexpectedOp(op, "ComputationalData")),
        }
    }

    pub fn parse_computational_data_ext(&mut self, op: ExtOp) -> Result<AmlData, AmlError> {
        match op {
            ExtOp::ReleaseOp => Ok(AmlData::Interger(u64::from(load_byte(self)?))),
            op => Err(AmlError::UnexpectedExtOp(op, "ComputationalData")),
        }
    }

    pub fn parse_data_object(&mut self, op: AmlOp) -> Result<AmlData, AmlError> {
        match op {
            AmlOp::PackageOp => Ok(AmlData::Package(Package::load(self)?)),
            AmlOp::VarPackageOp => Ok(AmlData::Package(Package::var_load(self)?)),
            _ => self.parse_computational_data(op),
        }
    }
    pub fn parse_data_ref_object(&mut self, op: AmlOp) -> Result<AmlData, AmlError> {
        self.parse_data_object(op)
    }

    pub fn get_or_exec(
        &mut self,
        path: NamePath,
        call_args: &[AmlData],
    ) -> Result<AmlData, AmlError> {
        let mut node = self.get_tlr();
        for seg in path.iter() {
            node = self.get_node(node)?.find_child(self, seg)?;
        }
        let name = &self.get_node(node)?.name.clone();
        let data = self.get_node(node)?.data.clone();
        if let AmlData::Method(method) = data {
            if usize::from(method.arg_c) != call_args.len() {
                Err(AmlError::InvalidArgCount(call_args.len(), method.arg_c))?;
            }

            let mut args = [const { AmlData::Null }; 7];
            for i in 0..core::cmp::min(call_args.len(), args.len()) {
                args[i] = call_args[i].clone();
            }

            method.call(self, &mut args, name)
        } else {
            Ok(data)
        }
    }

    pub fn exec_term_obj(&mut self) -> Result<AmlData, AmlError> {
        let op = match self.next_opcode() {
            Ok(op) => op,
            Err(AmlError::EndOfStream) => Err(AmlError::TermEnd)?,
            Err(e) => Err(e)?,
        };

        serial_println!("call_sp {}", sp());
        match op {
            AmlOp::BreakOp => Err(AmlError::Break),
            AmlOp::ReturnOp => self.exec_term_arg(),
            AmlOp::AliasOp => {
                let alias = NameString::exec(self)?;
                let target = self.resolve_path()?.unwrap_or(self.top_level_node);
                self.push_node(alias, AmlData::ObjRef(target))?;
                Ok(AmlData::Null)
            }
            AmlOp::ScopeOp => {
                let start = self.stream().index();
                let length = self.decode_pkg_length()?;
                let end = start + length;
                let name = NameString::exec(self)?;

                match self.push_node(name, AmlData::Scope) {
                    Ok(scope) => self.exec_within(scope, end)?,
                    Err(AmlError::NoChildNodeBy(_, _)) => self.stream.index = end,
                    Err(e) => Err(e)?,
                }

                Ok(AmlData::Null)
            }
            AmlOp::NameOp => {
                let name = NameString::exec(self)?;
                let next_op = self.next_opcode()?;
                let data = self.parse_data_ref_object(next_op)?;
                self.push_node(name, data)?;
                Ok(AmlData::Null)
            }
            AmlOp::MethodOp => {
                let end = self.stream().index() + self.decode_pkg_length()?;
                let name = NameString::exec(self)?;
                let method_flags = load_byte(self)?;
                let data = self
                    .stream()
                    .substream_untill(end)
                    .ok_or(AmlError::ScopeTooLong(end))?;
                let data = AmlData::Method(Method::new(data, method_flags));
                self.push_node(name, data)?;
                self.stream().index = end + 1;
                Ok(AmlData::Null)
            }
            AmlOp::WhileOp => {
                let end = self.stream().index() + self.decode_pkg_length()?;
                let data = self
                    .stream()
                    .substream_untill(end)
                    .ok_or(AmlError::ScopeTooLong(end))?;
                let stream_save = self.stream().clone();
                self.stream = data;

                while self.exec_term_arg()?.interger()? != 0 {
                    match self.exec_term_obj() {
                        Ok(_) | Err(AmlError::TermEnd) => (),
                        Err(AmlError::Break) => break,
                        Err(e) => Err(e)?,
                    }
                    self.stream.index = stream_save.index();
                }
                self.stream = stream_save;
                self.stream.index = end;
                Ok(AmlData::Null)
            }
            AmlOp::IfOp => {
                let end = self.stream().index() + self.decode_pkg_length()?;
                let res = self.exec_term_arg()?.interger()?;
                if res != 0 {
                    let mut data = self
                        .stream()
                        .substream_untill(end)
                        .ok_or(AmlError::ScopeTooLong(end))?;
                    self.swp_byte_stream(&mut data);
                    loop {
                        match self.exec_term_obj() {
                            Ok(_) => (),
                            Err(AmlError::TermEnd) => break,
                            Err(e) => Err(e)?,
                        }
                    }
                    self.swp_byte_stream(&mut data);
                    self.stream.index = end;
                    if matches!(self.peek_opcode(), Ok(AmlOp::ElseOp)) {
                        let _ = self.next();
                        let len = self.decode_pkg_length()?;
                        self.stream.index += len;
                    }
                } else {
                    self.stream.index = end;
                    if matches!(self.peek_opcode(), Ok(AmlOp::ElseOp)) {
                        let _ = self.next();
                        let end = self.stream().index() + self.decode_pkg_length()?;
                        let mut data = self
                            .stream()
                            .substream_untill(end)
                            .ok_or(AmlError::ScopeTooLong(end))?;
                        self.swp_byte_stream(&mut data);
                        loop {
                            match self.exec_term_obj() {
                                Ok(_) => (),
                                Err(AmlError::TermEnd) => break,
                                Err(e) => Err(e)?,
                            }
                        }
                        self.swp_byte_stream(&mut data);
                        self.stream.index = end;
                    }
                }

                Ok(AmlData::Null)
            }

            AmlOp::ExtOpPrefix => {
                //extended args
                let op = self.peek_ext_opcode()?;

                match op {
                    ExtOp::AquireOp => {
                        self.next()?;
                        let mutex_obj = self.resolve_super_name()?;
                        let timeout = load_word(self)?;
                        let timeout = Instant::now() + Duration::from_millis(u64::from(timeout));
                        todo!("Implement mutexing");
                    }

                    ExtOp::OpRegionOp => {
                        self.next()?;
                        let name = NameString::exec(self)?;
                        let region = load_byte(self)?;
                        let region_space = RegionSpace::parse(region)
                            .ok_or(AmlError::InvalidData(region, "RegionOpTermObject"))?;
                        let offset = self.exec_term_arg()?.interger()?;
                        let length = self.exec_term_arg()?.interger()?;
                        let data =
                            AmlData::NamedObject(super::named_objects::NamedObjects::OpRegionOp {
                                region_space,
                                offset,
                                length,
                            });
                        self.push_node(name, data)?;
                        Ok(AmlData::Null)
                    }

                    ExtOp::FieldOp => {
                        self.next()?;
                        let length = self.decode_pkg_length()?;
                        let namestring = self.resolve_path()?;
                        let field_flags = load_byte(self)?;
                        todo!("Implment actual hardware access!");
                    }

                    ExtOp::DeviceOpList => {
                        self.next()?;
                        let start = self.stream.index();
                        let length = self.decode_pkg_length()?;
                        let name = NameString::exec(self)?;
                        let device = self.push_node(name, AmlData::Device)?;

                        self.exec_within(device, start + length)?;
                        Ok(AmlData::Null)
                    }
                    _ => self.parse_expr_opcode_ext(op),
                }
            }
            op => self.parse_expr_opcode(op),
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
}

#[derive(Debug, Clone)]
pub struct Buffer {
    buffer: Box<[u8]>,
}

impl ByteCodeObject for Buffer {
    fn load(ctx: &mut AmlVM) -> Result<Self, AmlError> {
        let start = ctx.stream().index();
        let len = ctx.decode_pkg_length()? as usize;
        let size = ctx.exec_term_arg()?.interger()?;
        let mut data = Box::new_zeroed_slice(
            usize::try_from(size)
                .ok()
                .ok_or(AmlError::ScopeTooLong(size as usize))?,
        );
        let buf_start = ctx.stream().index() - start;
        for i in 0..(len - buf_start) {
            data.get_mut(i as usize)
                .ok_or(AmlError::IndexOutOfBounds(i))?
                .write(ctx.next()?);
        }
        Ok(Buffer {
            buffer: unsafe { data.assume_init() },
        })
    }
}
