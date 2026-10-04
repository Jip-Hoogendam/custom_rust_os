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
        AmlError, ByteCodeInstruction, Resolvable,
        data_objects::{AmlString, DataObj, Package, load_byte, load_dword, load_qword, load_word},
        named_objects::{NamePath, NameSeg, NameString, RegionSpace},
        namenode::{AmlData, Method, NameNode},
        opcodes::{AmlOp, AnyAmlOp, ExtOp},
        stack_items::AmlStack,
    },
    collections::typed_arena::{TypedArena, TypedArenaAllocation},
    serial_println,
};

#[derive(Clone, Debug)]
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
    pub stack: AmlStack,
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
            stack: AmlStack::new(stack_size),

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

    pub fn exec(&mut self) -> Result<(), AmlStackFrame> {
        loop {
            match self.stack.pop_unresolved() {
                Ok(Some(StackItem::Return)) if self.stream.done() => break,
                Ok(Some(item)) => self.resolve_stack_item(item),
                Ok(None) => {
                    if self.stream.done() {
                        match self.stack.pop_scoped() {
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
                            Ok(None) => break,
                            Ok(_) => Err(AmlStackFrame {
                                error: AmlError::StackCorruption,
                                stream: self.stream.clone(),
                            })?,
                            Err(e) => Err(AmlStackFrame {
                                error: e,
                                stream: self.stream.clone(),
                            })?,
                        }
                    } else {
                        self.exec_term_obj().map_err(|e| AmlStackFrame {
                            error: e,
                            stream: self.stream.clone(),
                        })?;
                    }
                    Ok(())
                }
                Err(e) => Err(e),
            }
            .map_err(|e| AmlStackFrame {
                error: e,
                stream: self.stream.clone(),
            })?;
        }
        Ok(())
    }

    #[inline]
    pub fn resolve_stack_item(&mut self, item: StackItem) -> Result<(), AmlError> {
        match item {
            StackItem::NamePath => self.resolve_path(),
            StackItem::SuperName => self.resolve_super_name(),
            StackItem::TermArg => self.exec_term_arg(),
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

    pub fn resolve_op(&mut self, op_index: usize) -> Result<(), AmlError> {
        match self.op_at(op_index)? {
            AmlOp::StoreOp => {
                let data = self.pop_data()?;
                let name_res = self.pop_data()?;
                self.set_node(name_res, data)?;
            }
            AmlOp::WhileOp => {
                let len = self.pop_data()?;
                let data = self.pop_data()?.interger()?;
                if data != 0 {
                    self.push_unresolved_op_at(op_index, &[StackItem::Pkglen, StackItem::TermArg])?;
                    self.stack.push(StackItem::Conditional {
                        ret_index: op_index + 1,
                        old_length: self.stream.length,
                    })?;
                    self.stream.length = op_index + len.interger()? as usize + 1;
                } else {
                    self.stream.index += len.interger()? as usize;
                }
            }
            AmlOp::ReturnOp => {
                let data = self.pop_data()?;

                loop {
                    match self.stack.pop()? {
                        Some(StackItem::MethodCall {
                            ret_index,
                            old_length,
                        }) => {
                            self.stream.index = ret_index;
                            self.stream.length = old_length;
                            break;
                        }
                        Some(_) => (),
                        None => Err(AmlError::StackCorruption)?,
                    }
                }
                self.stack.push(StackItem::Data(data))?;
                self.stack.push(StackItem::Return)?;
            }
            AmlOp::IfOp => {
                let len = self.pop_data()?.interger()? as usize;
                let pkg_len = Self::pkg_len_bytes(len);

                let mut orgiginal_index = self.stream.index;
                self.stream.index = op_index + len + pkg_len;

                let data = self.pop_data()?.interger()?;
                if data != 0 {
                    //true

                    //extend if else op is after this if
                    if matches!(self.peek_opcode(), Ok(AmlOp::ElseOp)) {
                        self.next()?;
                        let start = self.stream.index;
                        let len = self.decode_pkg_length()?;
                        self.stream.index = start + len;
                    }
                    core::mem::swap(&mut orgiginal_index, &mut self.stream.index);
                    self.stack.push(StackItem::Conditional {
                        ret_index: orgiginal_index,
                        old_length: self.stream.length,
                    })?;
                    self.stream.length = op_index + len + pkg_len;
                } else {
                    //false
                    if matches!(self.peek_opcode(), Ok(AmlOp::ElseOp)) {
                        self.next()?;
                        let _ = self.decode_pkg_length()?;
                    }
                }
            }
            AmlOp::DecrementOp => {
                let name_res = self.pop_data()?;
                let data = self.mutate_node(name_res, |d| {
                    Ok(AmlData::Interger(d.interger()?.wrapping_sub(1)))
                })?;
                self.stack.push_data(data)?;
            }
            AmlOp::IncrementOp => {
                let name_res = self.pop_data()?;
                let data = self.mutate_node(name_res, |d| {
                    Ok(AmlData::Interger(d.interger()?.wrapping_add(1)))
                })?;
                self.stack.push_data(data)?;
            }
            op => Err(AmlError::UnexpectedOp(op, "Resolve op"))?,
        }
        Ok(())
    }

    pub fn exec_term_obj(&mut self) -> Result<(), AmlError> {
        match self.next_opcode()? {
            AmlOp::ReturnOp => self.push_unresolved_op(&[StackItem::TermArg]),
            AmlOp::BreakOp => {
                while let Some(data) = self.stack.pop_scoped()? {
                    //if the return index is higher then the current index then it is a if function
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
                let end = self.stream().index() + self.decode_pkg_length()?;
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
            op => self.parse_expr_opcode(op),
        }
    }

    pub fn resolve_path(&mut self) -> Result<(), AmlError> {
        let opcode = self.peek_opcode()?;
        let obj_ref = match opcode {
            AmlOp::ZeroOp => {
                self.next()?;
                self.get_tlr()
            }
            AmlOp::DualNameOp => {
                self.next()?;
                let name = NameSeg::exec(self)?;
                let child = self.get_tln()?.seek_scope(self, &name)?;
                let name = NameSeg::exec(self)?;
                self.get_node(child)?.find_child(self, &name)?
            }
            AmlOp::MultiNameOp => {
                self.next()?;
                let seg_count = self.next()?;
                let first_name = NameSeg::exec(self)?;
                let mut node = self.get_tln()?.seek_scope(self, &first_name)?;
                for _ in 1..seg_count {
                    let name = NameSeg::exec(self)?;
                    node = self.get_node(node)?.find_child(self, &name)?;
                }
                node
            }
            b if matches!(b as u8, b'A'..=b'Z' | b'_') => {
                let name = NameSeg::exec(self)?;
                self.get_tln()?.seek_scope(self, &name)?
            }
            op => Err(AmlError::UnexpectedOp(op, "NamePath"))?,
        };
        self.stack.push_data(AmlData::ObjRef(obj_ref))
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
                AmlData::Arg(arg)
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
                AmlData::Local(local)
            }
            _ => return self.resolve_path(),
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
                let raw_ref = self.pop_data()?.interger()?;
                AmlData::ObjRef(ObjRef::from_raw_usize(raw_ref as usize).ok_or(AmlError::NullRef)?)
            }
            _ => return self.resolve_simple_name(),
        };
        self.stack.push_data(res)
    }

    pub fn resolve_target(&mut self) -> Result<(), AmlError> {
        let op = self.peek_opcode()?;
        let res = match op {
            AmlOp::ZeroOp => AmlData::Null,
            _ => return self.resolve_super_name(),
        };
        self.stack.push_data(res)
    }

    fn get_data_mut(&mut self, name_res: AmlData) -> Result<&mut AmlData, AmlError> {
        Ok(match name_res {
            AmlData::ObjRef(obj_ref) => &mut self.mut_node(obj_ref)?.data,
            AmlData::Arg(arg) => self.get_arg(arg)?,
            AmlData::Local(local) => self.get_local(local)?,
            AmlData::Debug => Err(AmlError::MutDebugObj)?,
            AmlData::Index(obj_ref, index) => {
                todo!();
            }
            AmlData::Null => &mut self.get_tln_mut()?.data,
            _ => Err(AmlError::NullObj)?,
        })
    }

    #[inline]
    fn mutate_node<F: FnOnce(&mut AmlData) -> Result<AmlData, AmlError>>(
        &mut self,
        name_res: AmlData,
        f: F,
    ) -> Result<AmlData, AmlError> {
        let data = self.get_data_mut(name_res)?;
        *data = f(data)?;
        Ok(data.clone())
    }

    fn exec_node<F: FnOnce(&mut AmlData, AmlData) -> Result<AmlData, AmlError>>(
        &mut self,
        name_res: AmlData,
        f: F,
        input: AmlData,
    ) -> Result<AmlData, AmlError> {
        let data = self.get_data_mut(name_res)?;
        *data = f(data, input)?;
        Ok(data.clone())
    }

    fn set_node(&mut self, name_res: AmlData, data: AmlData) -> Result<(), AmlError> {
        let old_data = match self.get_data_mut(name_res) {
            Ok(data) => data,
            Err(AmlError::MutDebugObj) => {
                return Ok(());
            }
            Err(e) => Err(e)?,
        };
        *old_data = data;
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
        //
        // match self.parse_data_object(op) {
        //     Ok(_) => return Ok(()),
        //     Err(AmlError::UnexpectedOp(_, _) | AmlError::UnexpectedExtOp(_, _)) => (),
        //     e => return e,
        // }

        match ArgObj::parse(op) {
            Ok(arg) => {
                let arg = self.get_arg(arg)?.clone();
                self.stack.push_data(arg)?;
                return Ok(());
            }
            Err(AmlError::InvalidData(_, _)) => (),
            Err(e) => Err(e)?,
        }

        match LocalObj::parse(op) {
            Ok(local) => {
                let local = self.get_local(local)?.clone();
                self.stack.push_data(local)?;
                return Ok(());
            }
            Err(AmlError::InvalidData(_, _)) => (),
            Err(e) => Err(e)?,
        }

        Err(AmlError::UnexpectedOp(op, "TermArg"))
    }
    pub fn parse_expr_opcode_ext(&mut self, op: ExtOp) -> Result<AmlData, AmlError> {
        match op {
            op => Err(AmlError::UnexpectedExtOp(op, "ExprParse Ext")),
        }
    }

    pub fn parse_expr_opcode(&mut self, op: AmlOp) -> Result<(), AmlError> {
        use StackItem::{SuperName, Target, TermArg};
        match op {
            AmlOp::AndOp
            | AmlOp::XorOp
            | AmlOp::OrOp
            | AmlOp::AddOp
            | AmlOp::SubtractOp
            | AmlOp::MultiplyOp => self.push_unresolved_op(&[TermArg, TermArg, Target]),
            AmlOp::DecrementOp | AmlOp::IncrementOp => self.push_unresolved_op(&[SuperName]),
            AmlOp::StoreOp => self.push_unresolved_op(&[TermArg, SuperName]),
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
            AmlOp::OnesOp => AmlData::Interger(0xFF),
            // AmlOp::BufferOp => AmlData::Buffer(Buffer::load(self)?),
            AmlOp::ExtOpPrefix => {
                let op = self.next_ext_opcode()?;
                return self.resolve_computational_data_ext(op);
            }

            op => Err(AmlError::UnexpectedOp(op, "ComputationalData"))?,
        };
        self.stack.push_data(res)
    }

    pub fn resolve_computational_data_ext(&mut self, op: ExtOp) -> Result<(), AmlError> {
        let res = match op {
            ExtOp::ReleaseOp => AmlData::Interger(u64::from(load_byte(self)?)),
            op => Err(AmlError::UnexpectedExtOp(op, "ComputationalData"))?,
        };
        self.stack.push_data(res)
    }

    // pub fn resolve_data_package(&mut self, op: ResOp) -> Result<(), AmlError> {
    //     match self.stream().op_at() {}
    // }
    //
    // #[inline]
    // pub fn parse_data_object(&mut self, op: AmlOp) -> Result<(), AmlError> {
    //     match op {
    //         AmlOp::PackageOp => AmlData::Package(Package::load(self)?),
    //         AmlOp::VarPackageOp => AmlData::Package(Package::var_load(self)?),
    //         _ => self.resolve_computational_data(op),
    //     }
    // }

    // #[inline]
    // pub fn parse_data_ref_object(&mut self, op: AmlOp) -> Result<(), AmlError> {
    //     self.parse_data_object(op)
    // }

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
    pub const fn swp_args(&mut self, new_args: &mut [AmlData; 7]) {
        core::mem::swap(&mut self.args, new_args);
    }
    #[inline]
    pub const fn swp_locals(&mut self, locals: &mut [AmlData; 8]) {
        core::mem::swap(&mut self.local, locals);
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

    fn get_local(&mut self, local: LocalObj) -> Result<&mut AmlData, AmlError> {
        self.local
            .get_mut(local.0 as usize)
            .ok_or(AmlError::InvalidLocalIndex(local.0))
    }
    fn get_arg(&mut self, arg: ArgObj) -> Result<&mut AmlData, AmlError> {
        self.args
            .get_mut(arg.0 as usize)
            .ok_or(AmlError::InvalidLocalIndex(arg.0))
    }

    /// Gets a obj refrence to current top level node from the context
    pub const fn get_tlr(&self) -> ObjRef {
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

    fn push_unresolved_op_at(&mut self, index: usize, args: &[StackItem]) -> Result<(), AmlError> {
        self.stack.op_arg_push(index, args)
    }

    ///Pushes a unresolved op to the stack
    fn push_unresolved_op(&mut self, args: &[StackItem]) -> Result<(), AmlError> {
        // -1 due to the stream index always sitting at the next item to be read.
        self.push_unresolved_op_at(self.stream.index - 1, args)
    }

    pub fn pop_data(&mut self) -> Result<AmlData, AmlError> {
        self.stack.pop_data()
    }
}

#[derive(Debug, Clone)]
pub struct Buffer {
    buffer: Box<[u8]>,
}
