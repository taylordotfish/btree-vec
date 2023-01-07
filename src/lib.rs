/*
 * Copyright (C) 2021-2023 taylor.fish <contact@taylor.fish>
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

#![cfg_attr(not(all(test, btree_vec_debug)), no_std)]
#![cfg_attr(
    any(feature = "allocator_api", has_allocator_api),
    feature(allocator_api)
)]
#![cfg_attr(feature = "dropck_eyepatch", feature(dropck_eyepatch))]
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
//! Example
//! -------
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
//! Crate features
//! --------------
//!
//! If the crate feature `dropck_eyepatch` is enabled, items in a [`BTreeVec`]
//! can contain references with the same life as the vector itself. This
//! requires Rust nightly, as the unstable language feature [`dropck_eyepatch`]
//! must be used.
//!
//! If the crate feature `allocator_api` is enabled, you can configure
//! [`BTreeVec`] with the unstable [`Allocator`] trait. Alternatively, if the
//! feature `allocator-fallback` is enabled, this crate will use the allocator
//! API provided by [allocator-fallback] instead of the standard library’s.
//!
//! [`dropck_eyepatch`]: https://github.com/rust-lang/rust/issues/34761
//! [allocator-fallback]: https://docs.rs/allocator-fallback
//!
//! [`Extend`]: core::iter::Extend
//! [`FromIterator`]: core::iter::FromIterator
//! [`Allocator`]: alloc::alloc::Allocator

extern crate alloc;

#[cfg(feature = "allocator_api")]
use alloc::alloc as allocator;

#[cfg(not(feature = "allocator_api"))]
#[cfg(feature = "allocator-fallback")]
use allocator_fallback as allocator;

#[cfg(not(any_allocator_api))]
#[path = "alloc_fallback.rs"]
mod allocator;

use alloc::boxed::Box;
use allocator::{Allocator, Global};
use core::fmt::{self, Debug, Formatter};
use core::iter::{ExactSizeIterator, FusedIterator};
use core::marker::PhantomData;
use core::ops::{Index, IndexMut};
use core::ptr::NonNull;

#[cfg(btree_vec_debug)]
#[allow(dead_code)]
pub mod debug;
mod insert;
mod node;
mod remove;
#[cfg(test)]
mod tests;
mod verified_alloc;

use insert::{insert, ItemInsertion};
use node::{LeafRef, Mutable, Node, NodeRef};
use node::{PrefixCast, PrefixPtr, PrefixRef};
use remove::remove;
use verified_alloc::VerifiedAlloc;

/// A growable array (vector) implemented as a B+ tree.
///
/// Provides non-amortized O(log n) random accesses, insertions, and removals,
/// and O(n) iteration.
///
/// `B` is the branching factor. It must be at least 3. The standard library
/// uses a value of 6 for its B-tree structures. Larger values are better when
/// `T` is smaller.
pub struct BTreeVec<T, const B: usize = 12, A: Allocator = Global> {
    root: Option<PrefixPtr<T, B>>,
    size: usize,
    alloc: VerifiedAlloc<A>,
    /// Lets dropck know that `T` may be dropped.
    phantom: PhantomData<Box<T>>,
}

// SAFETY: `BTreeVec` owns its data, so it can be sent to another thread.
unsafe impl<T, const B: usize, A> Send for BTreeVec<T, B, A>
where
    T: Send,
    A: Allocator,
{
}

// SAFETY: `BTreeVec` owns its data and provides access to it only through
// standard borrows.
unsafe impl<T, const B: usize, A> Sync for BTreeVec<T, B, A>
where
    T: Sync,
    A: Allocator,
{
}

fn leaf_for<T, const B: usize, R>(
    mut root: PrefixRef<T, B, R>,
    mut index: usize,
) -> (LeafRef<T, B, R>, usize) {
    loop {
        let node = match root.cast() {
            PrefixCast::Leaf(node) => return (node, index),
            PrefixCast::Internal(node) => node,
        };
        let last = node.length() - 1;
        let mut sizes = node.sizes.iter().copied().take(last);
        let index = sizes
            .position(|size| {
                if let Some(n) = index.checked_sub(size) {
                    index = n;
                    false
                } else {
                    true
                }
            })
            .unwrap_or(last);
        root = node.into_child(index);
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

impl<T, A: Allocator> BTreeVec<T, 12, A> {
    #[cfg_attr(
        not(any(feature = "allocator_api", feature = "allocator-fallback")),
        doc(hidden)
    )]
    /// Creates a new [`BTreeVec`] with the given allocator. Note that this
    /// function is implemented only for the default value of `B`; see
    /// [`Self::create_in`] for an equivalent that works with all values of
    /// `B`.
    pub fn new_in(alloc: A) -> Self {
        Self::create_in(alloc)
    }
}

impl<T, const B: usize> BTreeVec<T, B> {
    /// Creates a new [`BTreeVec`]. This function exists because
    /// [`BTreeVec::new`] is implemented only for the default value of `B`.
    pub fn create() -> Self {
        Self::create_in(Global)
    }
}

impl<T, const B: usize, A: Allocator> BTreeVec<T, B, A> {
    #[cfg_attr(
        not(any(feature = "allocator_api", feature = "allocator-fallback")),
        doc(hidden)
    )]
    /// Creates a new [`BTreeVec`] with the given allocator. This function
    /// exists because [`BTreeVec::new_in`] is implemented only for the default
    /// value of `B`.
    pub fn create_in(alloc: A) -> Self {
        assert!(B >= 3);
        // SAFETY:
        //
        // * All nodes are allocated by `alloc`, either via the calls to
        //  `insert` and `LeafRef::alloc` in `Self::insert`. Nodes are
        //  deallocated in two places: via the call to `remove` in
        //  `Self::remove`, and via the call to `NodeRef::destroy` in
        //  `Self::drop`. In both of these cases, `alloc` is provided as the
        //  allocator with which to deallocate the nodes.
        //
        // * When `alloc` (`Self.alloc`) is dropped, `Self::drop` will have
        //   run, which destroys all nodes. If `alloc`'s memory is reused
        //   (e.g., via `mem::forget`), the only way this can happen is if the
        //   operation that made its memory able to be reused applied to the
        //   entire `BTreeVec`. Thus, all allocated nodes will become
        //   inaccessible as they are not exposed via any public APIs,
        //   guaranteeing that they will never be accessed.
        let alloc = unsafe { VerifiedAlloc::new(alloc) };
        Self {
            root: None,
            size: 0,
            alloc,
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
        self.root.get_or_insert_with(|| {
            LeafRef::alloc(&self.alloc).into_prefix().as_ptr()
        });
        // SAFETY: `BTreeVec` uses `NodeRef`s in accordance with standard
        // borrowing rules, so there are no existing references.
        let (leaf, index) = unsafe { self.leaf_for_mut(index) };
        let root = insert(
            ItemInsertion {
                node: leaf,
                index,
                item,
                root_size: self.size,
            },
            &self.alloc,
        );
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
        let (root, item) = remove(leaf, index, &self.alloc);
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
            remaining: self.len(),
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
            remaining: self.len(),
            phantom: PhantomData,
        }
    }
}

impl<T, const B: usize, A> Default for BTreeVec<T, B, A>
where
    A: Allocator + Default,
{
    fn default() -> Self {
        Self::create_in(A::default())
    }
}

impl<T, const B: usize, A: Allocator> Index<usize> for BTreeVec<T, B, A> {
    type Output = T;

    fn index(&self, index: usize) -> &T {
        self.get(index).unwrap()
    }
}

impl<T, const B: usize, A: Allocator> IndexMut<usize> for BTreeVec<T, B, A> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        self.get_mut(index).unwrap()
    }
}

impl<T: Debug, const B: usize, A: Allocator> Debug for BTreeVec<T, B, A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

// SAFETY: This `Drop` impl does not directly or indirectly access any data in
// any `T`, except for calling its destructor (see [1]), and `Self` contains a
// `PhantomData<Box<T>>` so dropck knows that `T` may be dropped (see [2]).
//
// [1]: https://doc.rust-lang.org/nomicon/dropck.html
// [2]: https://forge.rust-lang.org/libs/maintaining-std.html
//      #is-there-a-manual-drop-implementation
#[cfg_attr(feature = "dropck_eyepatch", add_syntax::prepend(unsafe))]
impl<#[cfg_attr(feature = "dropck_eyepatch", may_dangle)] T, const B: usize, A>
    Drop for BTreeVec<T, B, A>
where
    A: Allocator,
{
    fn drop(&mut self) {
        if let Some(root) = self.root {
            // SAFETY: `BTreeVec` uses `NodeRef`s in accordance with
            // standard borrowing rules, so there are no existing
            // references.
            unsafe { NodeRef::new_mutable(root) }.destroy(&self.alloc);
        }
    }
}

fn nth<T, const B: usize, R>(
    leaf: LeafRef<T, B, R>,
    index: usize,
    mut n: usize,
) -> Option<(LeafRef<T, B, R>, usize)> {
    if let Some(new) = n.checked_sub(leaf.length() - index) {
        n = new;
    } else {
        return Some((leaf, index + n));
    };
    let mut child_index = leaf.index();
    let mut parent = leaf.into_parent().ok()?;
    loop {
        let sizes = parent.sizes[..parent.length()].iter().copied();
        for (i, size) in sizes.enumerate().skip(child_index + 1) {
            if let Some(new) = n.checked_sub(size) {
                n = new;
            } else {
                return Some(leaf_for(parent.into_child(i), n));
            }
        }
        child_index = parent.index();
        parent = parent.into_parent().ok()?;
    }
}

/// An iterator over the items in a [`BTreeVec`].
pub struct Iter<'a, T, const B: usize> {
    leaf: Option<LeafRef<T, B>>,
    index: usize,
    remaining: usize,
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

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let (leaf, i) = nth(self.leaf.take()?, self.index, n)?;
        self.index = i + 1;
        Some(self.leaf.insert(leaf).into_child(i))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T, const B: usize> FusedIterator for Iter<'_, T, B> {}

impl<T, const B: usize> ExactSizeIterator for Iter<'_, T, B> {
    fn len(&self) -> usize {
        let (lower, upper) = self.size_hint();
        debug_assert_eq!(Some(lower), upper);
        lower
    }
}

impl<T, const B: usize> Clone for Iter<'_, T, B> {
    fn clone(&self) -> Self {
        Self {
            leaf: self.leaf,
            index: self.index,
            remaining: self.remaining,
            phantom: self.phantom,
        }
    }
}

// SAFETY: This type yields immutable references to items in the vector, so it
// can be `Send` as long as `T` is `Sync` (which means `&T` is `Send`).
unsafe impl<T: Sync, const B: usize> Send for Iter<'_, T, B> {}

// SAFETY: This type has no `&self` methods that access shared data or fields
// with non-`Sync` interior mutability, but `T` must be `Sync` to match the
// `Send` impl, since this type implements `Clone`, effectively allowing it to
// be sent.
unsafe impl<T: Sync, const B: usize> Sync for Iter<'_, T, B> {}

impl<'a, T, const B: usize, A> IntoIterator for &'a BTreeVec<T, B, A>
where
    A: Allocator,
{
    type Item = &'a T;
    type IntoIter = Iter<'a, T, B>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// A mutable iterator over the items in a [`BTreeVec`].
pub struct IterMut<'a, T, const B: usize> {
    leaf: Option<LeafRef<T, B, Mutable>>,
    index: usize,
    remaining: usize,
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

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let (leaf, i) = nth(self.leaf.take()?, self.index, n)?;
        self.index = i + 1;
        // SAFETY: Extending the lifetime to `'a` is okay because `'a` doesn't
        // outlive the `BTreeVec` and we won't access this index again for the
        // life of the iterator.
        Some(unsafe {
            NonNull::from(self.leaf.insert(leaf).child_mut(i)).as_mut()
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T, const B: usize> FusedIterator for IterMut<'_, T, B> {}

impl<T, const B: usize> ExactSizeIterator for IterMut<'_, T, B> {
    fn len(&self) -> usize {
        let (lower, upper) = self.size_hint();
        debug_assert_eq!(Some(lower), upper);
        lower
    }
}

// SAFETY: This type yields mutable references to items in the vector, so it
// can be `Send` as long as `T` is `Send`. `T` doesn't need to be `Sync`
// because no other iterator that yields items from the vector can exist at the
// same time as this iterator.
unsafe impl<T: Send, const B: usize> Send for IterMut<'_, T, B> {}

// SAFETY: This type has no `&self` methods that access any fields.
unsafe impl<T, const B: usize> Sync for IterMut<'_, T, B> {}

impl<'a, T, const B: usize, A> IntoIterator for &'a mut BTreeVec<T, B, A>
where
    A: Allocator,
{
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T, B>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

/// An owning iterator over the items in a [`BTreeVec`].
pub struct IntoIter<T, const B: usize, A: Allocator = Global> {
    leaf: Option<LeafRef<T, B, Mutable>>,
    length: usize,
    index: usize,
    remaining: usize,
    _tree: BTreeVec<T, B, A>,
}

impl<T, const B: usize, A: Allocator> Iterator for IntoIter<T, B, A> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let mut leaf = self.leaf.as_mut()?;
        if self.index == self.length {
            self.leaf = self.leaf.take().unwrap().into_next().ok();
            leaf = self.leaf.as_mut()?;
            self.index = 0;
            self.length = leaf.length();
            leaf.set_zero_length();
        }
        let index = self.index;
        self.index += 1;
        self.remaining -= 1;
        // SAFETY: We haven't taken the item at `index` yet.
        Some(unsafe { leaf.take_raw_child(index).assume_init() })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T, const B: usize, A: Allocator> FusedIterator for IntoIter<T, B, A> {}

impl<T, const B: usize> ExactSizeIterator for IntoIter<T, B> {
    fn len(&self) -> usize {
        let (lower, upper) = self.size_hint();
        debug_assert_eq!(Some(lower), upper);
        lower
    }
}

// SAFETY: This type owns the items in the vector, so it can be `Send` as long
// as `T` is `Send`.
unsafe impl<T, const B: usize, A> Send for IntoIter<T, B, A>
where
    T: Send,
    A: Allocator,
{
}

// SAFETY: This type has no `&self` methods that access any fields.
unsafe impl<T, const B: usize, A: Allocator> Sync for IntoIter<T, B, A> {}

impl<T, const B: usize, A: Allocator> Drop for IntoIter<T, B, A> {
    fn drop(&mut self) {
        let mut leaf = if let Some(leaf) = self.leaf.take() {
            leaf
        } else {
            return;
        };
        for i in self.index..self.length {
            // SAFETY: We haven't taken the item at `index` yet.
            unsafe {
                leaf.take_raw_child(i).assume_init();
            }
        }
    }
}

impl<T, const B: usize, A: Allocator> IntoIterator for BTreeVec<T, B, A> {
    type Item = T;
    type IntoIter = IntoIter<T, B, A>;

    fn into_iter(mut self) -> Self::IntoIter {
        // SAFETY: `BTreeVec` uses `NodeRef`s in accordance with standard
        // borrowing rules, so because we own the `BTreeVec`, there are no
        // existing references.
        let leaf = self.root.map(|_| unsafe { self.leaf_for_mut(0) }.0);
        IntoIter {
            index: 0,
            length: leaf.as_ref().map_or(0, |leaf| leaf.length()),
            leaf,
            remaining: self.len(),
            _tree: self,
        }
    }
}
