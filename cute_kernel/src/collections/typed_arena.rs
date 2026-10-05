use core::{
    num::NonZeroUsize,
    ops::{Index, IndexMut},
};

use alloc::vec::Vec;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypedArenaAllocation(Option<NonZeroUsize>);

impl TypedArenaAllocation {
    #[must_use]
    pub fn from_raw_usize(value: usize) -> Option<Self> {
        Some(Self(Some(NonZeroUsize::try_from(value).ok()?)))
    }

    #[must_use]
    #[inline]
    pub const fn null() -> Self {
        Self(None)
    }

    #[inline]
    #[must_use]
    pub const fn is_null(&self) -> bool {
        self.0.is_none()
    }

    #[must_use]
    pub fn usize(&self) -> usize {
        if let Some(index) = self.0 {
            index.into()
        } else {
            0
        }
    }
}

///A simple arena allocator that dynamically grows. This expects another global allocator to already
///exist before allocating. This is basically a very thin wrapper around a vec.
pub struct TypedArena<T> {
    heap: Vec<T>,
}

impl<T> Default for TypedArena<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T> TypedArena<T> {
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self { heap: Vec::new() }
    }

    ///Inserts data into the arena. The refrence must be captured or else the allocation is lost.
    #[inline]
    pub fn instert(&mut self, data: T) -> TypedArenaAllocation {
        self.heap.push(data);
        TypedArenaAllocation(Some(NonZeroUsize::try_from(self.heap.len()).unwrap()))
    }

    ///Gets a mutable refrence to the data in the arena
    #[inline]
    pub fn get_mut(&mut self, alloc: TypedArenaAllocation) -> Option<&mut T> {
        self.heap.get_mut(alloc.0?.get() - 1usize)
    }

    //Gets a refence to the data in the arena
    #[inline]
    #[must_use]
    pub fn get(&self, alloc: TypedArenaAllocation) -> Option<&T> {
        self.heap.get(alloc.0?.get() - 1usize)
    }

    ///Finds a calloction based off the given function
    pub fn find(&self, f: impl Fn(&T) -> bool) -> Option<TypedArenaAllocation> {
        Some(TypedArenaAllocation(Some(
            NonZeroUsize::try_from(
                self.heap
                    .iter()
                    .enumerate()
                    .find(|(_index, data)| f(data))?
                    .0
                    + 1,
            )
            .unwrap(),
        )))
    }

    pub fn iter_mut(&mut self) -> impl Iterator {
        self.heap.iter_mut()
    }

    pub fn iter(&self) -> impl Iterator {
        self.heap.iter()
    }
}

impl<T> IndexMut<TypedArenaAllocation> for TypedArena<T> {
    fn index_mut(&mut self, index: TypedArenaAllocation) -> &mut Self::Output {
        let Some(data) = self.get_mut(index) else {
            panic!("Index not found!");
        };
        data
    }
}

impl<T> Index<TypedArenaAllocation> for TypedArena<T> {
    type Output = T;
    fn index(&self, index: TypedArenaAllocation) -> &Self::Output {
        let Some(data) = self.get(index) else {
            panic!("Index not found!");
        };
        data
    }
}
