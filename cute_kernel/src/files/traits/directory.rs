use crate::files::{
    path::PathParts,
    traits::{FSNode, file::File},
};
use alloc::{boxed::Box, vec::Vec};
use core::ops::DerefMut;

pub trait Directory {
    fn delete(&mut self, name: &str) {
        let list = self.list();
        let Some(index) = list.iter().position(|file| file.name() == name) else {
            return;
        };
        list.swap_remove(index);
    }

    fn find_node(&mut self, name: &str, mut path: PathParts) -> Option<&mut Box<dyn FSNode>> {
        for file in self.list() {
            if file.name() == name {
                if let Some(name) = path.next() {
                    return file.deref_mut().get_inode().find_node(name, path);
                }
                return Some(file);
            }
        }
        None
    }

    fn open_file(&mut self, name: &str) -> Option<&dyn File> {
        self.list()
            .iter_mut()
            .find(|file| file.name() == name)
            .map(|f| f.as_mut().get_inode().open_file())?
    }

    fn create_directory(&mut self, name: &str);
    fn create_file(&mut self, name: &str);

    fn list(&mut self) -> &mut Vec<Box<dyn FSNode>>;
}
