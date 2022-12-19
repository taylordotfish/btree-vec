/*
 * Copyright (C) 2022 taylor.fish <contact@taylor.fish>
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

use crate::Allocator;
use core::ops::Deref;

pub struct VerifiedAlloc<A>(A);

impl<A: Allocator> VerifiedAlloc<A> {
    /// # Safety
    ///
    /// * You must guarantee that `alloc`, accessible via [`Self::deref`], will
    ///   not be used to deallocate memory that was not allocated by `alloc`.
    ///
    ///   This means that the use of [`VerifiedAlloc`] must be kept to code
    ///   paths whose deallocation behavior is known and can be trusted.
    ///
    /// * You must ensure that the returned [`VerifiedAlloc`] will not be
    ///   dropped and its memory will not be reused (e.g., via [`mem::forget`])
    ///   unless at least one of the following is true:
    ///
    ///   * No pointers or references derived from memory allocated by `alloc`
    ///     exist.
    ///   * No pointers or references derived from memory allocated by `alloc`
    ///     will ever be accessed after the [`VerifiedAlloc`] is dropped or its
    ///     memory is reused.
    ///
    /// [`mem::forget`]: core::mem::forget
    pub unsafe fn new(alloc: A) -> Self {
        Self(alloc)
    }
}

impl<A> Deref for VerifiedAlloc<A> {
    type Target = A;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
