use alloc::{string::String, vec};
use core::{fmt::Display, str::Split};

///Interface of creating and working with file paths
pub struct Path {
    inner: String,
}

impl Default for Path {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for Path {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "/{}", self.inner)
    }
}

pub type PathParts<'a> = Split<'a, char>;

impl Path {
    //Initilizes the path to the root directory
    #[must_use]
    pub const fn new() -> Self {
        Self {
            inner: String::new(),
        }
    }

    //Splits the path into parts
    #[must_use]
    #[inline]
    pub fn parts(&self) -> PathParts<'_> {
        self.inner.split('/')
    }

    //Cannonoclizes the path to a actual location
    #[must_use]
    pub fn cannonaclize(&self) -> Self {
        let mut path = vec![];
        for part in self.parts() {
            if part == ".." {
                path.pop();
            } else {
                path.push(part.trim());
            }
        }
        Self::from(path.join("/"))
    }
}

impl From<&'_ str> for Path {
    fn from(value: &'_ str) -> Self {
        Self {
            inner: String::from(value),
        }
    }
}

impl From<String> for Path {
    fn from(value: String) -> Self {
        Self { inner: value }
    }
}
