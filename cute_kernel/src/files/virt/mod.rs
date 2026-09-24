use alloc::string::{String, ToString};

use crate::files::traits::{FSNode, Inode};

pub mod directory;
pub mod file;

pub struct VirtNode {
    name: String,
    inode: Inode,
}

impl VirtNode {
    #[must_use]
    pub fn new(name: &str, inode: Inode) -> Self {
        Self {
            name: name.to_string(),
            inode,
        }
    }
}

impl FSNode for VirtNode {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn get_inode(&mut self) -> &mut Inode {
        &mut self.inode
    }
}
