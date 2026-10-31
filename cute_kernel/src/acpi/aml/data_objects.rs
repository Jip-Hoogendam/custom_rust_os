use core::cell::RefCell;

use alloc::{boxed::Box, rc::Rc, string::String, vec};

use crate::{
    acpi::aml::{
        AmlError, Resolvable,
        executor::{self, AmlVM, ByteStream},
        named_objects::NameString,
        namenode::AmlData,
    },
    serial_println,
};

#[derive(Debug, PartialEq, Eq)]
pub struct AmlString(pub Rc<RefCell<Box<[u8]>>>);

impl Clone for AmlString {
    fn clone(&self) -> Self {
        serial_println!("CALL");
        Self(Rc::clone(&self.0))
    }
}

impl AmlString {
    pub fn exec(ctx: &mut AmlVM) -> Result<Self, AmlError> {
        let mut string = vec![];
        loop {
            match ctx.next()? {
                0x00 => break Ok(Self(Rc::new(RefCell::new(string.into_boxed_slice())))),
                char @ 0x01..0x7F => string.push(char),
                data => break Err(AmlError::InvalidData(data, "AmlString")),
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.borrow().is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.borrow().len()
    }

    pub fn get(&self, index: usize) -> Option<u8> {
        self.0.borrow().get(index).map(|b| *b)
    }

    pub fn from_string(mut string: String) -> Result<AmlString, AmlError> {
        for (index, byte) in string.bytes().enumerate() {
            match byte {
                0x00 => {
                    string.drain(index..);
                    break;
                }
                0x01..0x7F => (),
                data => return Err(AmlError::InvalidData(data, "AmlString")),
            }
        }
        Ok(Self(Rc::new(RefCell::new(
            string.into_boxed_str().into_boxed_bytes(),
        ))))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, AmlError> {
        let mut string = vec![];
        for byte in bytes {
            match byte {
                0x00 => return Ok(Self(Rc::new(RefCell::new(string.into_boxed_slice())))),
                char @ 0x01..0x7F => string.push(*char),
                data => return Err(AmlError::InvalidData(*data, "AmlString")),
            }
        }
        Ok(Self(Rc::new(RefCell::new(string.into_boxed_slice()))))
    }

    pub fn to_buf(self) -> Buffer {
        Buffer {
            buffer: self.0.borrow().clone(),
        }
    }
}

#[repr(u8)]
#[derive(Debug)]
pub enum DataObj {
    ByteConst(u8),
    WordConst(u16),
    DWordConst(u32),
    QWordConst(u64),
    String(AmlString),
    ConstZeroObj,
    ConstOneObj,
    ConstOnesObj,
    Buffer(Buffer),
    Package(Package),
    NameString(NameString),
}

pub fn load_byte(stream: &mut AmlVM) -> Result<u8, AmlError> {
    stream.next()
}

pub fn load_word(ctx: &mut AmlVM) -> Result<u16, AmlError> {
    Ok(u16::from_le_bytes([load_byte(ctx)?, load_byte(ctx)?]))
}

pub fn load_dword(ctx: &mut AmlVM) -> Result<u32, AmlError> {
    Ok(u32::from_le_bytes([
        load_byte(ctx)?,
        load_byte(ctx)?,
        load_byte(ctx)?,
        load_byte(ctx)?,
    ]))
}

pub fn load_qword(ctx: &mut AmlVM) -> Result<u64, AmlError> {
    Ok(u64::from_le_bytes([
        load_byte(ctx)?,
        load_byte(ctx)?,
        load_byte(ctx)?,
        load_byte(ctx)?,
        load_byte(ctx)?,
        load_byte(ctx)?,
        load_byte(ctx)?,
        load_byte(ctx)?,
    ]))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Buffer {
    pub buffer: Box<[u8]>,
}

impl Buffer {
    pub fn new(pkg_len: u64, mut size: u64, stream: &mut ByteStream) -> Result<Self, AmlError> {
        if pkg_len > size {
            size = pkg_len;
        }

        let mut buffer = Box::new_zeroed_slice(size as usize);
        for byte in 0..(pkg_len as usize) {
            buffer[byte].write(stream.next().ok_or(AmlError::EndOfStream)?);
        }

        Ok(Buffer {
            buffer: unsafe { buffer.assume_init() },
        })
    }
}

#[derive(Debug, Clone)]
pub struct Package {
    elements: Box<[AmlData]>,
}

impl Package {
    pub fn new(len: usize) -> Result<Self, AmlError> {
        if len > 255 {
            Err(AmlError::PackageTooLong(len as u64))?
        }
        let mut elements = Box::new_uninit_slice(len);
        for element in &mut elements {
            element.write(AmlData::Null);
        }
        Ok(Package {
            elements: unsafe { elements.assume_init() },
        })
    }

    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut AmlData> {
        self.elements.get_mut(index)
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<&AmlData> {
        self.elements.get(index)
    }
}
