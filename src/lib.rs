/*
 * Copyright (C) 2021-2022 taylor.fish <contact@taylor.fish>
 *
 * This file is part of btree-vec.
 *
 * btree-vec is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * btree-vec is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with btree-vec. If not, see <https://www.gnu.org/licenses/>.
 */

#![cfg_attr(not(test), no_std)]
#![deny(unsafe_op_in_unsafe_fn)]

//! This crate provides a growable array (vector) implemented using a B-tree
//! (more specifically, a B+ tree). It provides non-amortized O(log n) random
//! accesses, insertions, and removals, as well as O(n) iteration. The
//! branching factor is also customizable.
//!
//! The design is similar to [unsorted counted B-trees][cb] as described by
//! Simon Tatham.
//!
//! [cb]: https://www.chiark.greenend.org.uk/~sgtatham/algorithms/cbtree.html
//!
//! For now, the vector supports insertions and removals only of single
//! elements, but bulk operations, including implementations of [`Extend`]
//! and [`FromIterator`], may be added in the future.
//!
//! # Example
//!
//! ```rust
//! # use btree_vec::BTreeVec;
//! let mut vec = BTreeVec::new();
//! for i in 0..20 {
//!     vec.push(i);
//! }
//! for i in 0..10 {
//!     assert!(vec.remove(i) == i * 2);
//! }
//! for i in 0..10 {
//!     assert!(vec[i] == i * 2 + 1);
//! }
//! for i in 0..10 {
//!     vec.insert(i * 2, i * 2);
//! }
//! assert!(vec.len() == 20);
//! for (i, n) in vec.iter().copied().enumerate() {
//!     assert!(i == n);
//! }
//! ```
//!
//! [`Extend`]: core::iter::Extend
//! [`FromIterator`]: core::iter::FromIterator

extern crate alloc;

use core::fmt::{self, Debug, Formatter};
use core::iter::FusedIterator;
use core::marker::PhantomData;
use core::ops::{Index, IndexMut};
use core::ptr::NonNull;

#[cfg(test)]
#[allow(dead_code)]
pub(crate) mod debug;
mod insert;
mod node;
mod remove;
#[cfg(test)]
mod tests;

use insert::insert;
use node::{LeafRef, NodeRef, PrefixRef};
use node::{Mutable, Node, PrefixCast, PrefixPtr, RefKind};
use remove::remove;

/// A growable array (vector) implemented as a B+ tree.
///
/// Provides non-amortized O(log n) random accesses, insertions, and removals,
/// and O(n) iteration.
///
/// `B` is the branching factor. It must be at least 3. The standard library
/// uses a value of 6 for its B-tree structures. Larger values are better when
/// `T` is smaller.
pub struct BTreeVec<T, const B: usize = 12> {
    root: Option<PrefixPtr<T, B>>,
    size: usize,
    phantom: PhantomData<T>,
}

// SAFETY: `BTreeVec` owns its data, so it can be sent to another thread.
unsafe impl<T: Send, const B: usize> Send for BTreeVec<T, B> {}

// SAFETY: `BTreeVec` owns its data and provides access to it only through
// standard borrows.
unsafe impl<T: Sync, const B: usize> Sync for BTreeVec<T, B> {}

fn leaf_for<T, const B: usize, R: RefKind>(
    mut root: PrefixRef<T, B, R>,
    mut index: usize,
) -> (LeafRef<T, B, R>, usize) {
    loop {
        root = match root.cast() {
            PrefixCast::Leaf(node) => return (node, index),
            PrefixCast::Internal(node) => {
                let last = node.length() - 1;
                let index = node
                    .sizes()
                    .iter()
                    .enumerate()
                    .take(last)
                    .find_map(|(i, size)| {
                        if let Some(n) = index.checked_sub(*size) {
                            index = n;
                            None
                        } else {
                            Some(i)
                        }
                    })
                    .unwrap_or(last);
                node.into_child(index)
            }
        }
    }
}

impl<T> BTreeVec<T> {
    /// Creates a new [`BTreeVec`]. Note that this function is implemented
    /// only for the default value of `B`; see [`Self::create`] for an
    /// equivalent that works with all values of `B`.
    pub fn new() -> Self {
        Self::create()
    }
}

impl<T, const B: usize> BTreeVec<T, B> {
    /// Creates a new [`BTreeVec`]. This function exists because
    /// [`BTreeVec::new`] is implemented only for the default value of `B`.
    pub fn create() -> Self {
        assert!(B >= 3);
        Self {
            root: None,
            size: 0,
            phantom: PhantomData,
        }
    }

    /// # Safety
    ///
    /// * There must not be any mutable references, including other
    ///   [`NodeRef`]s where `R` is [`Mutable`], to any data accessible via the
    ///   returned [`NodeRef`].
    ///
    /// [`Mutable`]: node::Mutable
    unsafe fn leaf_for(&self, index: usize) -> (LeafRef<T, B>, usize) {
        // SAFETY: Caller guarantees safety.
        leaf_for(unsafe { NodeRef::new(self.root.unwrap()) }, index)
    }

    /// # Safety
    ///
    /// There must be no other references, including [`NodeRef`]s, to any data
    /// accessible via the returned [`NodeRef`].
    unsafe fn leaf_for_mut(
        &mut self,
        index: usize,
    ) -> (LeafRef<T, B, Mutable>, usize) {
        // SAFETY: Caller guarantees safety.
        leaf_for(unsafe { NodeRef::new_mutable(self.root.unwrap()) }, index)
    }

    /// Gets the length of the vector.
    pub fn len(&self) -> usize {
        self.size
    }

    /// Checks whether the vector is empty.
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Gets the item at `index`, or [`None`] if no such item exists.
    pub fn get(&self, index: usize) -> Option<&T> {
        (index < self.size).then(|| {
            // SAFETY: `BTreeVec` uses `NodeRef`s in accordance with
            // standard borrowing rules, so there are no existing mutable
            // references.
            let (leaf, index) = unsafe { self.leaf_for(index) };
            leaf.into_child(index)
        })
    }

    /// Gets a mutable reference to the item at `index`, or [`None`] if no such
    /// item exists.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        (index < self.size).then(|| {
            // SAFETY: `BTreeVec` uses `NodeRef`s in accordance with
            // standard borrowing rules, so there are no existing references.
            let (leaf, index) = unsafe { self.leaf_for_mut(index) };
            leaf.into_child_mut(index)
        })
    }

    /// Gets the first item in the vector, or [`None`] if the vector is empty.
    pub fn first(&self) -> Option<&T> {
        self.get(0)
    }

    /// Gets a mutable reference to the first item in the vector, or [`None`]
    /// if the vector is empty.
    pub fn first_mut(&mut self) -> Option<&mut T> {
        self.get_mut(0)
    }

    /// Gets the last item in the vector, or [`None`] if the vector is empty.
    pub fn last(&self) -> Option<&T> {
        self.size.checked_sub(1).and_then(|s| self.get(s))
    }

    /// Gets a mutable reference to the last item in the vector, or [`None`] if
    /// the vector is empty.
    pub fn last_mut(&mut self) -> Option<&mut T> {
        self.size.checked_sub(1).and_then(move |s| self.get_mut(s))
    }

    /// Inserts `item` at `index`.
    ///
    /// # Panics
    ///
    /// Panics if `index` is greater than [`self.len()`](Self::len).
    pub fn insert(&mut self, index: usize, item: T) {
        assert!(index <= self.size);
        self.root
            .get_or_insert_with(|| LeafRef::alloc().into_prefix().as_ptr());
        // SAFETY: `BTreeVec` uses `NodeRef`s in accordance with standard
        // borrowing rules, so there are no existing references.
        let (leaf, index) = unsafe { self.leaf_for_mut(index) };
        let root = insert(leaf, index, item, self.size);
        self.root = Some(root.as_ptr());
        self.size += 1;
    }

    /// Inserts `item` at the end of the vector.
    pub fn push(&mut self, item: T) {
        self.insert(self.size, item);
    }

    /// Removes and returns the item at `index`.
    ///
    /// # Panics
    ///
    /// Panics if `index` is not less than [`self.len()`](Self::len).
    pub fn remove(&mut self, index: usize) -> T {
        assert!(index < self.size);
        // SAFETY: `BTreeVec` uses `NodeRef`s in accordance with
        // standard borrowing rules, so there are no existing references.
        let (leaf, index) = unsafe { self.leaf_for_mut(index) };
        let (root, item) = remove(leaf, index);
        self.root = Some(root.as_ptr());
        self.size -= 1;
        item
    }

    /// Removes and returns the last item in the vector, or [`None`] if the
    /// vector is empty.
    pub fn pop(&mut self) -> Option<T> {
        self.size.checked_sub(1).map(|s| self.remove(s))
    }

    /// Gets an iterator that returns references to each item in the vector.
    pub fn iter(&self) -> Iter<'_, T, B> {
        // SAFETY: `BTreeVec` uses `NodeRef`s in accordance with standard
        // borrowing rules, so there are no existing mutable references.
        Iter {
            leaf: self.root.map(|_| unsafe { self.leaf_for(0) }.0),
            index: 0,
            phantom: PhantomData,
        }
    }

    /// Gets an iterator that returns mutable references to each item in the
    /// vector.
    pub fn iter_mut(&mut self) -> IterMut<'_, T, B> {
        // SAFETY: `BTreeVec` uses `NodeRef`s in accordance with standard
        // borrowing rules, so there are no existing references.
        IterMut {
            leaf: self.root.map(|_| unsafe { self.leaf_for_mut(0) }.0),
            index: 0,
            phantom: PhantomData,
        }
    }
}

impl<T, const B: usize> Default for BTreeVec<T, B> {
    fn default() -> Self {
        Self::create()
    }
}

impl<T, const B: usize> Index<usize> for BTreeVec<T, B> {
    type Output = T;

    fn index(&self, index: usize) -> &T {
        self.get(index).unwrap()
    }
}

impl<T, const B: usize> IndexMut<usize> for BTreeVec<T, B> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        self.get_mut(index).unwrap()
    }
}

impl<T: Debug, const B: usize> Debug for BTreeVec<T, B> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T, const B: usize> Drop for BTreeVec<T, B> {
    fn drop(&mut self) {
        if let Some(root) = self.root {
            // SAFETY: `BTreeVec` uses `NodeRef`s in accordance with standard
            // borrowing rules, so there are no existing references.
            unsafe { NodeRef::new_mutable(root) }.destroy();
        }
    }
}

/// An iterator over the items in a [`BTreeVec`].
pub struct Iter<'a, T, const B: usize> {
    leaf: Option<LeafRef<T, B>>,
    index: usize,
    phantom: PhantomData<&'a T>,
}

impl<'a, T, const B: usize> Iterator for Iter<'a, T, B> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let mut leaf = self.leaf?;
        if self.index == leaf.length() {
            self.leaf = self.leaf.take().unwrap().into_next().ok();
            leaf = self.leaf?;
            self.index = 0;
        }
        let index = self.index;
        self.index += 1;
        Some(leaf.into_child(index))
    }
}

impl<'a, T, const B: usize> FusedIterator for Iter<'a, T, B> {}

impl<'a, T, const B: usize> IntoIterator for &'a BTreeVec<T, B> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T, B>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// An mutable iterator over the items in a [`BTreeVec`].
pub struct IterMut<'a, T, const B: usize> {
    leaf: Option<LeafRef<T, B, Mutable>>,
    index: usize,
    phantom: PhantomData<&'a mut T>,
}

impl<'a, T, const B: usize> Iterator for IterMut<'a, T, B> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        let mut leaf = self.leaf.as_mut()?;
        if self.index == leaf.length() {
            self.leaf = self.leaf.take().unwrap().into_next().ok();
            leaf = self.leaf.as_mut()?;
            self.index = 0;
        }
        let index = self.index;
        self.index += 1;
        // SAFETY: Extending the lifetime to `'a` is okay because `'a` doesn't
        // outlive the `BTreeVec` and we won't access this index again for the
        // life of the iterator.
        Some(unsafe { NonNull::from(leaf.child_mut(index)).as_mut() })
    }
}

impl<'a, T, const B: usize> FusedIterator for IterMut<'a, T, B> {}

impl<'a, T, const B: usize> IntoIterator for &'a mut BTreeVec<T, B> {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T, B>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

/// An owning iterator over the items in a [`BTreeVec`].
pub struct IntoIter<T, const B: usize> {
    // Note: the field order is important here -- we need `_tree` to occur
    // after `leaf` so there are no existing `NodeRef`s when `_tree` gets
    // dropped (see `BTreeVec::drop`).
    leaf: Option<LeafRef<T, B, Mutable>>,
    length: usize,
    index: usize,
    _tree: BTreeVec<T, B>,
}

impl<T, const B: usize> Iterator for IntoIter<T, B> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let mut leaf = self.leaf.as_mut()?;
        if self.index == self.length {
            self.leaf = self.leaf.take().unwrap().into_next().ok();
            leaf = self.leaf.as_mut()?;
            self.index = 0;
            self.length = leaf.length();
        }
        let index = self.index;
        self.index += 1;
        // SAFETY: We haven't taken the item at `index` yet.
        Some(unsafe { leaf.take_raw_child(index).assume_init() })
    }
}

impl<T, const B: usize> FusedIterator for IntoIter<T, B> {}

impl<T, const B: usize> IntoIterator for BTreeVec<T, B> {
    type Item = T;
    type IntoIter = IntoIter<T, B>;

    fn into_iter(mut self) -> Self::IntoIter {
        // SAFETY: `BTreeVec` uses `NodeRef`s in accordance with standard
        // borrowing rules, so because we own the `BTreeVec`, there are no
        // existing references.
        let leaf = self.root.map(|_| unsafe { self.leaf_for_mut(0) }.0);
        IntoIter {
            index: 0,
            length: leaf.as_ref().map_or(0, |leaf| leaf.length()),
            leaf,
            _tree: self,
        }
    }
}
