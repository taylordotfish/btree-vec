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

use super::{InternalRef, Mutable, NodeRef, PrefixRef, RefKind};
use super::{Node, NodeKind, Prefix, PrefixPtr, SplitStrategy};
use core::marker::PhantomData as Pd;
use core::mem;
use core::ptr::NonNull;

#[repr(C, align(2))]
pub struct InternalNode<T, const B: usize> {
    prefix: Prefix<T, B>,
    length: usize,
    children: [Option<PrefixPtr<T, B>>; B],
    sizes: [usize; B],
}

impl<T, const B: usize> Drop for InternalNode<T, B> {
    fn drop(&mut self) {
        for child in &mut self.children[..self.length] {
            let mut child = NodeRef(child.take().unwrap(), Pd);
            child.parent.set(None);
            child.destroy();
        }
    }
}

impl<T, const B: usize> InternalNode<T, B> {
    /// # Safety
    ///
    /// To be used only by [`NodeRef::alloc`].
    pub unsafe fn new() -> Self {
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
    ) -> NodeRef<Self, Mutable> {
        let (left, right) = strategy.sizes(B);
        assert!(self.length == B);
        let mut new = InternalRef::alloc();
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
        let ptr = NonNull::from(&mut *self);
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
                prefix.parent.set(Some(ptr));
                prefix.index = length + i;
                *self_child = Some(other_child);
            });
        self.length += other.length;
        other.length = 0;
    }

    pub fn simple_insert(
        &mut self,
        i: usize,
        mut item: (PrefixRef<T, B, Mutable>, usize),
    ) {
        let length = self.length;
        assert!(length < B);
        let ptr = NonNull::from(&mut *self);
        item.0.index = i;
        item.0.parent.set(Some(ptr));
        self.children[i..length + 1].rotate_right(1);
        self.sizes[i..length + 1].rotate_right(1);
        self.children[i] = Some(item.0.0);
        self.sizes[i] = item.1;
        self.length += 1;
        for i in (i + 1)..=length {
            self.child_mut(i).0.index = i;
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
            self.child_mut(i).0.index = i;
        }
        let mut child = NodeRef(self.children[length - 1].take().unwrap(), Pd);
        let size = mem::replace(&mut self.sizes[length - 1], 0);
        child.parent.set(None);
        child.index = 0;
        self.length -= 1;
        (child, size)
    }

    /// This method always returns pointers to initialized children
    /// (or `None`).
    fn child_ptr(&self, i: usize) -> Option<PrefixPtr<T, B>> {
        // Children at 0..self.length are always initialized.
        self.children[..self.length].get(i).copied().flatten()
    }

    pub fn try_child(&self, i: usize) -> Option<(&Prefix<T, B>, usize)> {
        // SAFETY: `Self::child_ptr` returns initialized children, and we
        // hand out references only according to standard borrow rules, so
        // we can dereference.
        self.child_ptr(i).map(|p| (unsafe { p.as_ref() }, self.sizes[i]))
    }

    pub fn try_child_mut(
        &mut self,
        i: usize,
    ) -> Option<(&mut Prefix<T, B>, &mut usize)> {
        // SAFETY: `Self::child_ptr` returns initialized children, and we
        // hand out references only according to standard borrow rules, so
        // we can dereference.
        self.child_ptr(i)
            .map(move |mut p| (unsafe { p.as_mut() }, &mut self.sizes[i]))
    }

    pub fn child(&self, i: usize) -> (&Prefix<T, B>, usize) {
        self.try_child(i).unwrap()
    }

    pub fn child_mut(&mut self, i: usize) -> (&mut Prefix<T, B>, &mut usize) {
        self.try_child_mut(i).unwrap()
    }

    pub fn sizes(&self) -> &[usize] {
        &self.sizes[..self.length]
    }

    pub fn size(&self) -> usize {
        self.sizes().iter().sum()
    }
}

// SAFETY: `Node` may be implemented by `InternalNode`.
unsafe impl<T, const B: usize> Node for InternalNode<T, B> {
    type Prefix = Prefix<T, B>;
    type Child = (PrefixRef<T, B, Mutable>, usize);

    unsafe fn new() -> Self {
        // SAFETY: Checked by caller.
        unsafe { InternalNode::<T, B>::new() }
    }

    fn item_size(item: &Self::Child) -> usize {
        item.1
    }

    fn prefix(&self) -> &Self::Prefix {
        &self.prefix
    }

    fn prefix_mut(&mut self) -> &mut Self::Prefix {
        &mut self.prefix
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

    fn simple_insert(&mut self, i: usize, item: Self::Child) {
        self.simple_insert(i, item);
    }

    fn simple_remove(&mut self, i: usize) -> Self::Child {
        self.simple_remove(i)
    }

    fn split(&mut self, strategy: SplitStrategy) -> NodeRef<Self, Mutable> {
        self.split(strategy)
    }

    fn merge(&mut self, other: &mut Self) {
        self.merge(other)
    }
}

impl<T, const B: usize, R: RefKind> NodeRef<InternalNode<T, B>, R> {
    pub fn into_child(self, i: usize) -> PrefixRef<T, B, R> {
        R::into_child(self, i)
    }
}

impl<T, const B: usize> NodeRef<InternalNode<T, B>> {
    pub fn child_ref(&self, i: usize) -> PrefixRef<T, B> {
        NodeRef(NonNull::from(self.child(i).0), Pd)
    }
}
