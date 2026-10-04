use alloc::{boxed::Box, vec};

use crate::{
    acpi::aml::{AmlError, executor::ObjRef, namenode::AmlData},
    serial_println,
};

#[derive(Debug, Clone)]
pub struct OpRes {
    index: usize,
}

#[derive(Debug, Clone)]
pub enum StackItem {
    Data(AmlData),
    TermArg,   //expect any aml data
    NamePath,  //Expect objref
    SuperName, //Expect objref
    Target,    //Expect objref
    Pkglen,
    Byte,

    Return,
    MethodCall {
        ret_index: usize,
        old_length: usize,
    },

    Conditional {
        ret_index: usize,
        old_length: usize,
    },

    Scope {
        old_length: usize,
        old_tlr: ObjRef,
    },

    UnreselvedOp {
        op_index: usize,
        arg_c: u8,
        arg_s: u8,
    },
    ResOp(usize),
    Null,
}

pub struct AmlStack {
    stack_ptr: usize,
    stack: Box<[StackItem]>,
}

impl AmlStack {
    pub fn new(size: usize) -> Self {
        Self {
            stack_ptr: 0,
            stack: vec![const { StackItem::Null }; size].into_boxed_slice(),
        }
    }

    pub const fn empty(&self) -> bool {
        self.stack_ptr == 0
    }

    pub fn push_data(&mut self, item: AmlData) -> Result<(), AmlError> {
        if matches!(item, AmlData::Null) {
            return Ok(()); //Dont push null data onto the stack!
        }
        if let Some(StackItem::UnreselvedOp { arg_s, .. }) = self.peek() {
            let index = *arg_s as usize;
            let index = self.stack_ptr - index - 1;
            return if let Some(curr) = self.stack.get_mut(index)
                && !matches!(curr, StackItem::Data(_))
            {
                *curr = StackItem::Data(item);
                Ok(())
            } else {
                Err(AmlError::StackCorruption)
            };
        }
        Ok(()) //Dont really need random data on the stack
    }

    ///Pushes data on the stack and into the correct place to work with.
    pub fn push(&mut self, item: StackItem) -> Result<(), AmlError> {
        *self
            .stack
            .get_mut(self.stack_ptr)
            .ok_or(AmlError::StackOverflow)? = item;
        self.stack_ptr += 1; //Cold day in hell if this overflows
        Ok(())
    }

    ///Pops data from the stack regardless of what is on the stack
    pub fn pop(&mut self) -> Result<Option<StackItem>, AmlError> {
        let Some(new_ptr) = self.stack_ptr.checked_sub(1) else {
            return Ok(None);
        };
        self.stack_ptr = new_ptr;
        let mut res = StackItem::Null;
        let data = self
            .stack
            .get_mut(self.stack_ptr)
            .ok_or(AmlError::StackOverflow)?;
        core::mem::swap(&mut res, data);

        Ok(Some(res))
    }

    ///Pops resolved data form the stack.
    pub fn pop_data(&mut self) -> Result<AmlData, AmlError> {
        if let Some(StackItem::Data(data)) = self.pop()? {
            Ok(data)
        } else {
            Err(AmlError::StackCorruption)
        }
    }

    pub fn pop_scoped(&mut self) -> Result<Option<StackItem>, AmlError> {
        if let Some(data) = self.peek() {
            match data {
                StackItem::MethodCall { .. }
                | StackItem::Scope { .. }
                | StackItem::Conditional { .. } => self.pop(),
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    ///Pops unresolved data from the stack to work with.
    pub fn pop_unresolved(&mut self) -> Result<Option<StackItem>, AmlError> {
        if let Some(data) = self.peek() {
            match data {
                StackItem::UnreselvedOp {
                    op_index,
                    arg_c,
                    arg_s,
                } => {
                    if arg_s < arg_c {
                        *arg_s += 1;
                        let index = *arg_s as usize;
                        let index = self.stack_ptr - index - 1;
                        Ok(self.stack.get(index).map(|x| x.clone()))
                    } else {
                        let data = Some(StackItem::ResOp(*op_index));
                        self.pop()?;
                        return Ok(data);
                    }
                }
                StackItem::MethodCall { .. }
                | StackItem::Scope { .. }
                | StackItem::Conditional { .. } => Ok(None),
                StackItem::Data(_) => Err(AmlError::StackCorruption),
                _ => self.pop(),
            }
        } else {
            Ok(None)
        }
    }

    ///Looks at the top of the stack and returns a mutable pointer to it
    fn peek(&mut self) -> Option<&mut StackItem> {
        let index = self.stack_ptr.checked_sub(1)?;
        self.stack.get_mut(index)
    }

    pub fn op_arg_push(&mut self, op_index: usize, args: &[StackItem]) -> Result<(), AmlError> {
        assert!(args.len() < u8::MAX as usize);
        for item in args.iter().rev() {
            self.push(item.clone())?;
        }
        self.push(StackItem::UnreselvedOp {
            op_index,
            arg_c: args.len() as u8,
            arg_s: 0u8,
        })
    }

    // pub fn push_op(&mut self, op: AmlOp) -> Result<(), AmlError> {
    //     use AmlOp::*;
    //     use StackItem::*;
    //     match op {
    //         LandOp | LorOp | LLlessOp | LgreaterOp | LequalOp => {
    //             self.op_arg_push(op, &[TermArg, TermArg])
    //         }
    //         AddOp | SubtractOp | XorOp | AndOp | OrOp | MultiplyOp => {
    //             self.op_arg_push(op, &[TermArg, TermArg, Target])
    //         }
    //         IfOp => self.op_arg_push(op, &[TermArg]),
    //         _ => Err(AmlError::UnexpectedOp(op, "Stack Op push")),
    //     }
    // }
}
