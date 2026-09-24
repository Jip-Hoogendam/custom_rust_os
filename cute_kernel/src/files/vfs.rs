use crate::files::{
        path::Path,
        traits::{directory::Directory, file::File},
        virt::directory::VirutalDirectory,
    };

///The internal virtual file system to support the mouting of drives and alot of other things
pub struct VirtualFileSystem {
    root: VirutalDirectory,
}

impl Default for VirtualFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualFileSystem {
    pub fn open(&mut self, path: &Path) -> Option<&dyn File> {
        let path = path.cannonaclize();
        let mut path_parts = path.parts();
        path_parts.next(); // Ignore everything before the \/
        let seek_name = path_parts.next()?;

        if path_parts.clone().next().is_some() {
            self.root
                .find_node(seek_name, path_parts)?
                .as_mut()
                .get_inode()
                .open_file()
        } else {
            self.root.open_file(seek_name)
        }
    }

    pub fn create_directory(&mut self, path: &Path, dir_name: &str) {
        let path = path.cannonaclize();
        let mut path_parts = path.parts();
        path_parts.next(); // Ignore everything before the \/

        if let Some(seek_name) = path_parts.next()
            && !seek_name.is_empty()
        {
            if let Some(node) = self.root
                .find_node(seek_name, path_parts) { node.as_mut().get_inode().create_directory(dir_name) }
        } else {
            self.root.create_directory(dir_name);
        }
    }

    pub fn create_file(&mut self, path: &Path, file_name: &str) {
        let path = path.cannonaclize();
        let mut path_parts = path.parts();
        path_parts.next(); // Ignore everything before the \/

        if let Some(seek_name) = path_parts.next()
            && !seek_name.is_empty()
        {
            if let Some(node) = self.root
                .find_node(seek_name, path_parts) { node.as_mut().get_inode().create_file(file_name) }
        } else {
            self.root.create_file(file_name);
        }
    }

    #[must_use]
    pub const fn new() -> Self {
        Self {
            root: VirutalDirectory::new(),
        }
    }
}
