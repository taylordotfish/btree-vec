/*
 * Copyright (C) 2021 taylor.fish <contact@taylor.fish>
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
//! (more specifically, a B+ tree). It provides O(log n) random accesses,
//! insertions, and removals, as well as O(n) iteration. The branching factor
//! is also customizable.
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

#[cfg(test)]
mod tests;

/// Defines [`BTreeVec`](vec::BTreeVec), a growable array implemented as a
/// B+ tree.
pub mod vec;

/// [`vec::BTreeVec`] with a default branching factor.
pub type BTreeVec<T> = vec::BTreeVec<T, 12>;
