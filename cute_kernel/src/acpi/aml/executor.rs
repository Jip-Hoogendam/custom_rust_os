use alloc::{boxed::Box, string::String, vec};
use core::{error::Error, fmt::Display, mem::transmute, time::Duration};

use crate::{
    acpi::aml::{
        AmlError, ByteCodeInstruction, ByteCodeObject,
        data_objects::{AmlString, DataObj, Package, load_byte, load_dword, load_qword, load_word},
        named_objects::{NamePath, NameSeg, NameString},
        namenode::{AmlData, NameNode},
        opcodes::{AmlOp, AnyAmlOp, ExtOp},
    },
    collections::typed_arena::{TypedArena, TypedArenaAllocation},
    println, serial_println,
    tables::gdt::SegmentSelector,
    time::Instant,
};

#[derive(Clone)]
pub struct ByteStream {
    bytes: *const u8,
    index: usize,
    length: usize,
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
    pub fn new_slice(&self, start: usize, length: usize) -> Option<Self> {
        if start <= self.index && self.index < length {
            let mut clone = self.clone();
            clone.length = core::cmp::min(start + length, self.length);
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
impl ByteCodeInstruction for ArgObj {
    fn exec(stream: &mut AmlVM) -> Result<Self, AmlError> {
        let op = stream.next()?;
        let index = if matches!(op, b'h'..=b'n') {
            op - b'h'
        } else {
            Err(AmlError::InvalidData(op))?
        };
        Ok(Self(index))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LocalObj(u8);

impl ByteCodeInstruction for LocalObj {
    fn exec(stream: &mut AmlVM) -> Result<Self, AmlError> {
        let op = stream.next()?;
        let index = if matches!(op, b'`'..=b'g') {
            op - b'`'
        } else {
            Err(AmlError::InvalidData(op))?
        };
        Ok(Self(index))
    }
}

pub type ObjRef = TypedArenaAllocation;
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
    pub index: usize,
}

impl Error for AmlStackFrame {}

impl Display for AmlStackFrame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "[AML EXEC ERROR]\nInstruction : {}\nbacktrace:\n{}",
            self.index, self.error
        )
    }
}

pub struct AmlVM {
    stream: ByteStream,
    top_level_node: ObjRef,
    heap: TypedArena<NameNode>,
    args: [AmlData; 7],
    local: [AmlData; 8],
}

impl AmlVM {
    pub fn new(start_point: *const u8, length: usize) -> Self {
        let mut heap = TypedArena::new();
        let root_obj = NameNode::new(NameSeg::root(), AmlData::Null);
        let top_level_node = heap.instert(root_obj);
        Self {
            stream: ByteStream::new(start_point, length),
            heap,
            top_level_node,
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
            match self.exect_op() {
                Ok(_) => (),
                Err(AmlError::TermEnd) => return Ok(()),
                Err(err) => {
                    return Err(AmlStackFrame {
                        error: err,
                        index: self.stream().index(),
                    });
                }
            }
        }
    }

    #[inline]
    #[must_use]
    pub fn stream(&mut self) -> &mut ByteStream {
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

    pub fn resolve_path(&mut self) -> Result<NameRes, AmlError> {
        let opcode = self.peek_opcode()?;
        match opcode {
            AmlOp::ZeroOp => {
                self.next()?;
                Ok(NameRes::Null)
            }
            AmlOp::DualNameOp => {
                self.next()?;
                let name = NameSeg::exec(self)?;
                let child = self.get_tln()?.find_child(self, &name)?;
                let name = NameSeg::exec(self)?;
                let child = self.get_node(child)?.find_child(self, &name)?;
                Ok(NameRes::Ref(child))
            }
            AmlOp::MultiNameOp => {
                self.next();
                let seg_count = self.next()?;
                let mut node = self.get_tlr();
                for _ in 0..seg_count {
                    let name = NameSeg::exec(self)?;
                    node = self.get_node(node)?.find_child(self, &name)?;
                }
                Ok(NameRes::Ref(node))
            }
            b if matches!(b as u8, b'A'..=b'Z' | b'_') => {
                let name = NameSeg::exec(self)?;
                Ok(NameRes::Ref(self.get_tln()?.find_child(self, &name)?))
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
            _ => self.resolve_path(),
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
            op => Err(AmlError::UnexpectedOp(op, "SuperName")),
        }
    }

    fn mutate_node(&mut self, name_res: NameRes, data: AmlData) -> Result<(), AmlError> {
        match name_res {
            NameRes::Ref(obj_ref) => {
                let node = self.mut_node(obj_ref)?;
                node.data = data;
                Ok(())
            }
            NameRes::Arg(arg) => {
                let arg = self.get_arg(arg)?;
                *arg = data;
                Ok(())
            }
            NameRes::Local(local) => {
                let local = self.get_local(local)?;
                *local = data;
                Ok(())
            }
            NameRes::Debug => {
                println!("AmlDebug: {:?}", data);
                Ok(())
            }
            NameRes::Index(obj_ref, index) => {
                todo!();
            }
            NameRes::Null => Err(AmlError::NullRef),
        }
    }

    fn push_node(&mut self, name: NameString, data: AmlData) -> Result<ObjRef, AmlError> {
        // if no top level node set it to this node instead
        if let NameString::Rel(NamePath::NameSeg(name), 0) = name {
            let obj_ref = self.heap.instert(NameNode::new(name, data));
            self.top_level_node = obj_ref;
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
                let node = self.get_node(node)?.find_child(self, &in_name)?;
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
    fn append_to_node(
        &mut self,
        parent: ObjRef,
        mut new_child: NameNode,
    ) -> Result<ObjRef, AmlError> {
        new_child.parent = Some(parent);
        let child_ref = self.heap.instert(new_child);
        let parent = self
            .heap
            .get_mut(parent)
            .ok_or(AmlError::InvalidObjectRef(parent))?;
        parent.children.push(child_ref);
        Ok(child_ref)
    }

    pub fn fn_for<D, F: Fn(&mut Self) -> Result<D, AmlError>>(
        &mut self,
        count: usize,
        start: usize,
        data_length: usize,
        f: F,
    ) -> Result<Box<[D]>, AmlError> {
        let mut stream = self
            .stream()
            .new_slice(start, data_length)
            .ok_or(AmlError::ScopeTooLong(data_length))?;
        core::mem::swap(&mut self.stream, &mut stream);

        let mut res = vec![];
        res.reserve_exact(count);

        for _ in 0..data_length {
            res.push(f(self)?)
        }

        self.stream = stream;
        self.stream.index += 1 + data_length;
        Ok(res.into_boxed_slice())
    }

    ///Executes data within the given scope
    fn exec_within(
        &mut self,
        obj_ref: ObjRef,
        start: usize,
        length: usize,
    ) -> Result<(), AmlError> {
        let mut stream = self
            .stream()
            .new_slice(start, length)
            .ok_or(AmlError::ScopeTooLong(length))?;

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
        self.stream.index += 1 + length;
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
        match op {
            AmlOp::PackageOp => Ok(AmlData::Package(Package::load(self)?)),
            AmlOp::ExtOpPrefix => {
                let op = self.next_ext_opcode()?;
                match op {
                    ExtOp::ReleaseOp => Ok(AmlData::Byte(load_byte(self)?)),
                    op => Err(AmlError::UnexpectedExtOp(op, "TermArg")),
                }
            }
            op => Err(AmlError::UnexpectedOp(op, "TermArg")),
        }
    }

    pub fn parse_computational_data(&mut self, op: AmlOp) -> Result<AmlData, AmlError> {
        match op {
            AmlOp::ZeroOp => Ok(AmlData::Byte(0x0)),
            AmlOp::OneOp => Ok(AmlData::Byte(0x1)),
            AmlOp::ByteOp => Ok(AmlData::Byte(load_byte(self)?)),
            AmlOp::WordOp => Ok(AmlData::Interger(u64::from(load_word(self)?))),
            AmlOp::DWordOp => Ok(AmlData::Interger(u64::from(load_dword(self)?))),
            AmlOp::QWordOp => Ok(AmlData::Interger(load_qword(self)?)),
            AmlOp::StringOp => Ok(AmlData::String(AmlString::exec(self)?)),
            AmlOp::OnesOp => Ok(AmlData::Byte(0xFF)),
            AmlOp::BufferOp => Ok(AmlData::Buffer(Buffer::exec(self)?)),
            AmlOp::ExtOpPrefix if self.peek_ext_opcode()? == ExtOp::ReleaseOp => {
                self.next()?;
                Ok(AmlData::Byte(load_byte(self)?))
            }
            op => Err(AmlError::UnexpectedOp(op, "ComputationalData")),
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

    pub fn exect_op(&mut self) -> Result<(), AmlError> {
        let op = match self.next_opcode() {
            Ok(op) => op,
            Err(AmlError::EndOfStream) => Err(AmlError::TermEnd)?,
            Err(e) => Err(e)?,
        };
        match op {
            AmlOp::ScopeOp => {
                let start = self.stream.index();
                let length = self.decode_pkg_length()?;
                let name = NameString::exec(self)?;

                let scope = self.push_node(name, AmlData::Scope)?;
                self.exec_within(scope, start, length as usize)?;
            }
            AmlOp::NameOp => {
                let name = NameString::exec(self)?;
                let next_op = self.next_opcode()?;
                let data = self.parse_data_ref_object(next_op)?;
                self.push_node(name, data)?;
            }

            AmlOp::ExtOpPrefix => {
                //extended args
                let op = self.next_ext_opcode()?;

                match op {
                    ExtOp::AquireOp => {
                        let mutex_obj = self.resolve_super_name()?;
                        let timeout = load_word(self)?;
                        let timeout = Instant::now() + Duration::from_millis(u64::from(timeout));
                        todo!()
                    }

                    ExtOp::DeviceOpList => {
                        let start = self.stream.index();
                        let length = self.decode_pkg_length()?;
                        let name = NameString::exec(self)?;
                        let device = self.push_node(name, AmlData::Device)?;

                        self.exec_within(device, start, length as usize)?;
                    }

                    op => Err(AmlError::UnexpectedExtOp(op, "TermObject"))?,
                }
            }
            op => Err(AmlError::UnexpectedOp(op, "TermObject"))?,
        }
        Ok(())
    }

    pub fn decode_pkg_length(&mut self) -> Result<u32, AmlError> {
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
            data => Err(AmlError::InvalidData(data))?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Buffer {
    buffer: Box<[u8]>,
}

impl ByteCodeInstruction for Buffer {
    fn exec(ctx: &mut AmlVM) -> Result<Self, AmlError> {
        let op = ctx.next_opcode()?;
        if op == AmlOp::BufferOp {
            let len = ctx.decode_pkg_length()?;
            let size = ctx.exec_term_arg()?.interger()?;
            todo!()
        } else {
            Err(AmlError::UnexpectedOp(op, "Buffer"))
        }
    }
}
