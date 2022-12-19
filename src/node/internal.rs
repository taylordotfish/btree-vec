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

use super::{InternalRef, Mutable, NodeRef, PrefixRef};
use super::{Node, NodeKind, Prefix, PrefixPtr, SplitStrategy};
use crate::{Allocator, VerifiedAlloc};
use core::marker::PhantomData as Pd;
use core::mem;

#[repr(C, align(2))]
pub struct InternalNode<T, const B: usize> {
    prefix: Prefix<T, B>,
    length: usize,
    children: [Option<PrefixPtr<T, B>>; B],
    pub sizes: [usize; B],
}

impl<T, const B: usize> InternalNode<T, B> {
    fn new() -> Self {
        Self {
            prefix: Prefix::new(NodeKind::Internal),
            length: 0,
            children: [(); B].map(|_| None),
            sizes: [0; B],
        }
    }

    pub fn split(
        &mut self,
        strategy: SplitStrategy,
        alloc: &VerifiedAlloc<impl Allocator>,
    ) -> NodeRef<Self, Mutable> {
        let (left, right) = strategy.sizes(B);
        assert!(self.length == B);
        let mut new = InternalRef::alloc(alloc);
        let ptr = new.0;
        new.sizes[..right].copy_from_slice(&self.sizes[left..]);
        self.children[left..]
            .iter_mut()
            .map(|c| c.take().unwrap())
            .zip(&mut new.children[..right])
            .enumerate()
            .for_each(|(i, (mut old_child, new_child))| {
                // SAFETY: We have the only reference to `old_child`, and this
                // type's invariants guarantee its validity.
                let prefix = unsafe { old_child.as_mut() };
                prefix.parent.set(Some(ptr));
                prefix.index = i;
                *new_child = Some(old_child);
            });
        self.length = left;
        new.length = right;
        new
    }

    pub fn merge(&mut self, other: &mut Self) {
        let length = self.length;
        assert!(length <= B / 2);
        assert!(other.length <= B / 2);
        let parent = self.child(0).parent;
        self.sizes[length..][..other.length]
            .copy_from_slice(&other.sizes[..other.length]);
        other.children[..other.length]
            .iter_mut()
            .map(|c| c.take().unwrap())
            .zip(&mut self.children[length..])
            .enumerate()
            .for_each(|(i, (mut other_child, self_child))| {
                // SAFETY: We have the only reference to `other_child`, and
                // this type's invariants guarantee its validity.
                let prefix = unsafe { other_child.as_mut() };
                prefix.parent = parent;
                prefix.index = length + i;
                *self_child = Some(other_child);
            });
        self.length += other.length;
        other.length = 0;
    }

    pub fn simple_insert(
        this: &mut NodeRef<Self, Mutable>,
        i: usize,
        mut item: (PrefixRef<T, B, Mutable>, usize),
    ) {
        let length = this.length;
        assert!(length < B);
        let ptr = this.0;
        item.0.index = i;
        item.0.parent.set(Some(ptr));
        this.children[i..length + 1].rotate_right(1);
        this.sizes[i..length + 1].rotate_right(1);
        this.children[i] = Some(item.0.0);
        this.sizes[i] = item.1;
        this.length += 1;
        for i in (i + 1)..=length {
            this.child_mut(i).index = i;
        }
    }

    pub fn simple_remove(
        &mut self,
        i: usize,
    ) -> (PrefixRef<T, B, Mutable>, usize) {
        let length = self.length;
        assert!(length > 0);
        self.children[i..length].rotate_left(1);
        self.sizes[i..length].rotate_left(1);
        for i in i..(length - 1) {
            self.child_mut(i).index = i;
        }
        let mut child = NodeRef(self.children[length - 1].take().unwrap(), Pd);
        let size = mem::replace(&mut self.sizes[length - 1], 0);
        child.parent.set(None);
        child.index = 0;
        self.length -= 1;
        (child, size)
    }

    /// This method always returns pointers to initialized, properly aligned
    /// children (or `None`).
    pub fn child_ptr(&self, i: usize) -> Option<PrefixPtr<T, B>> {
        // Children at 0..self.length are always initialized.
        self.children[..self.length].get(i).copied().flatten()
    }

    pub fn child(&self, i: usize) -> &Prefix<T, B> {
        // SAFETY: `Self::child_ptr` returns initialized children, and we
        // hand out references only according to standard borrow rules, so
        // we can dereference.
        unsafe { self.child_ptr(i).unwrap().as_ref() }
    }

    pub fn child_mut(&mut self, i: usize) -> &mut Prefix<T, B> {
        // SAFETY: See `Self::child`.
        unsafe { self.child_ptr(i).unwrap().as_mut() }
    }

    pub fn size(&self) -> usize {
        self.sizes.iter().sum()
    }

    pub fn destroy_children(&mut self, alloc: &VerifiedAlloc<impl Allocator>) {
        self.children[..mem::take(&mut self.length)]
            .iter_mut()
            .map(|child| NodeRef(child.take().unwrap(), Pd))
            .for_each(|mut child| {
                child.parent.set(None);
                child.destroy(alloc);
            });
    }
}

impl<T, const B: usize> Node for InternalNode<T, B> {
    type Prefix = Prefix<T, B>;
    type Child = (PrefixRef<T, B, Mutable>, usize);

    fn new(_: super::node_ref_alloc::Token) -> Self {
        Self::new()
    }

    fn item_size(item: &Self::Child) -> usize {
        item.1
    }

    fn prefix(&self) -> &Self::Prefix {
        &self.prefix
    }

    fn size(&self) -> usize {
        self.size()
    }

    fn length(&self) -> usize {
        self.length
    }

    fn index(&self) -> usize {
        self.prefix.index
    }

    fn simple_insert(
        this: &mut NodeRef<Self, Mutable>,
        i: usize,
        item: Self::Child,
    ) {
        Self::simple_insert(this, i, item);
    }

    fn simple_remove(&mut self, i: usize) -> Self::Child {
        self.simple_remove(i)
    }

    fn split(
        &mut self,
        strategy: SplitStrategy,
        alloc: &VerifiedAlloc<impl Allocator>,
    ) -> NodeRef<Self, Mutable> {
        self.split(strategy, alloc)
    }

    fn merge(&mut self, other: &mut Self) {
        self.merge(other)
    }

    fn destroy_children(&mut self, alloc: &VerifiedAlloc<impl Allocator>) {
        self.destroy_children(alloc);
    }
}

impl<T, const B: usize, R> NodeRef<InternalNode<T, B>, R> {
    pub fn into_child(self, i: usize) -> PrefixRef<T, B, R> {
        NodeRef(self.child_ptr(i).unwrap(), Pd)
    }
}

impl<T, const B: usize> NodeRef<InternalNode<T, B>> {
    #[allow(dead_code)]
    pub fn child_ref(&self, i: usize) -> PrefixRef<T, B> {
        NodeRef(self.child_ptr(i).unwrap(), Pd)
    }
}
