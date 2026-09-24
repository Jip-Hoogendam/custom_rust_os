use alloc::boxed::Box;

use crate::files::{
    path::PathParts,
    traits::{directory::Directory, file::File, mount::Mount},
};

pub mod directory;
pub mod file;
pub mod mount;

pub enum Inode {
    Directory(Box<dyn Directory>),
    File(Box<dyn File>),
    Mount(Box<dyn Mount>),
}

impl Inode {
    pub fn find_node(&mut self, name: &str, path: PathParts) -> Option<&mut Box<dyn FSNode>> {
        match self {
            Self::Directory(dir) => dir.as_mut().find_node(name, path),
            Self::Mount(dir) => dir.as_mut().find_node(name, path),
            Self::File(_) => None,
        }
    }

    pub fn delete(&mut self, name: &str) {
        match self {
            Self::Directory(dir) => dir.as_mut().delete(name),
            Self::Mount(dir) => dir.as_mut().delete(name),
            Self::File(_) => (),
        }
    }

    pub fn open_file(&mut self) -> Option<&dyn File> {
        match self {
            Self::File(file) => Some(file.open()),
            Self::Directory(_) | Self::Mount(_) => None,
        }
    }

    pub fn create_file(&mut self, name: &str) {
        match self {
            Self::File(_) => (),
            Self::Mount(mnt_dir) => mnt_dir.create_file(name),
            Self::Directory(mnt_dir) => mnt_dir.create_file(name),
        }
    }

    pub fn create_directory(&mut self, name: &str) {
        match self {
            Self::File(_) => (),
            Self::Mount(mnt_dir) => mnt_dir.create_directory(name),
            Self::Directory(mnt_dir) => mnt_dir.create_directory(name),
        }
    }
}

pub trait FSNode {
    fn name(&self) -> &str;
    fn get_inode(&mut self) -> &mut Inode;
}
