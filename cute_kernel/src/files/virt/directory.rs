use alloc::{boxed::Box, vec, vec::Vec};

use crate::{
    files::{
        traits::{FSNode, Inode, directory::Directory},
        virt::{VirtNode, file::VirtualFile},
    },
    serial_println,
};

pub struct VirutalDirectory {
    files: Vec<Box<dyn FSNode>>,
}

impl VirutalDirectory {
    pub const fn new() -> Self {
        Self { files: vec![] }
    }
}

impl Directory for VirutalDirectory {
    fn list(&mut self) -> &mut Vec<Box<dyn FSNode>> {
        &mut self.files
    }

    fn create_file(&mut self, name: &str) {
        let file = Box::new(VirtualFile::new());
        let node = VirtNode::new(name, Inode::File(file));
        self.files.push(Box::new(node));
    }

    fn create_directory(&mut self, name: &str) {
        let file = Box::new(Self::new());
        let node = VirtNode::new(name, Inode::Directory(file));
        self.files.push(Box::new(node));
    }
}
