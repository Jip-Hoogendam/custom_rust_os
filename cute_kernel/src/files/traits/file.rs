use alloc::{boxed::Box, rc::Rc, vec::Vec};

use crate::{files::traits::FSNode, sync::Mutex};

pub enum FileError {
    FileDoesNotExist,
}

pub trait File {
    fn open(&self) -> &dyn File;
    fn write(&self, buffer: &[u8], offset: usize);
    fn read(&self, buffer: &mut [u8], offset: usize);
}
