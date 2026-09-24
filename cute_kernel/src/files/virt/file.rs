use core::cmp::min;

use alloc::{vec, vec::Vec};

use crate::{files::traits::file::File, sync::Mutex};

pub struct VirtualFile {
    contents: Mutex<Vec<u8>>,
}

impl Default for VirtualFile {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualFile {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            contents: Mutex::new(vec![]),
        }
    }
}

impl File for VirtualFile {
    fn open(&self) -> &dyn File {
        self
    }

    fn read(&self, buffer: &mut [u8], offset: usize) {
        let contents = self.contents.lock();
        let len = min(buffer.len(), contents.len() - offset);
        for i in 0..len {
            buffer[i] = contents[i + offset];
        }
    }

    fn write(&self, buffer: &[u8], offset: usize) {
        let mut contents = self.contents.lock();
        let replace_len = min(contents.len() - offset, buffer.len());

        let (within, outside) = buffer.split_at(replace_len);
        let (_, slice) = contents.split_at_mut(offset);
        slice.clone_from_slice(within);

        if buffer.len() > replace_len {
            contents.extend_from_slice(outside);
        }
    }
}
