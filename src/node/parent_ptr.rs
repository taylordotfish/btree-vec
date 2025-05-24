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

use super::{InternalNode, NodeKind};
use core::ptr::NonNull;
use tagged_pointer::TaggedPtr;

pub(super) struct ParentPtr<T, const B: usize>(
    TaggedPtr<InternalNode<T, B>, 1>,
);

impl<T, const B: usize> Clone for ParentPtr<T, B> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T, const B: usize> Copy for ParentPtr<T, B> {}

impl<T, const B: usize> ParentPtr<T, B> {
    fn sentinel() -> NonNull<InternalNode<T, B>> {
        #[repr(align(2))]
        struct Align2(#[allow(dead_code)] u16);

        static SENTINEL: Align2 = Align2(0);
        NonNull::from(&SENTINEL).cast()
    }

    pub fn new(kind: NodeKind) -> Self {
        Self(TaggedPtr::new(Self::sentinel(), kind as usize))
    }

    pub fn get(&self) -> Option<NonNull<InternalNode<T, B>>> {
        let ptr = self.0.ptr();
        (ptr != Self::sentinel()).then(|| ptr.cast())
    }

    pub fn set(&mut self, ptr: Option<NonNull<InternalNode<T, B>>>) {
        self.0.set_ptr(ptr.unwrap_or_else(Self::sentinel));
    }

    pub fn kind(&self) -> NodeKind {
        NodeKind::VARIANTS[self.0.tag()]
    }
}
