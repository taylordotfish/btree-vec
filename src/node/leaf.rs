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

use super::{Mutable, NodeRef, Prefix, RefKind};
use super::{Node, NodeKind, SplitStrategy};
use core::marker::PhantomData as Pd;
use core::mem::{self, MaybeUninit};
use core::ptr::{self, NonNull};

#[repr(C)]
pub struct LeafNode<T, const B: usize> {
    prefix: Prefix<T, B>,
    length: usize,
    children: [MaybeUninit<T>; B],
    next: Option<NonNull<Self>>,
}

impl<T, const B: usize> Drop for LeafNode<T, B> {
    fn drop(&mut self) {
        for child in &mut self.children[..self.length] {
            // SAFETY: Items at 0..length are always initialized.
            unsafe {
                mem::replace(child, MaybeUninit::uninit()).assume_init();
            }
        }
    }
}

impl<T, const B: usize> LeafNode<T, B> {
    /// # Safety
    ///
    /// To be used only by [`NodeRef::alloc`].
    pub unsafe fn new() -> Self {
        Self {
            prefix: Prefix::new(NodeKind::Leaf),
            length: 0,
            children: [(); B].map(|_| MaybeUninit::uninit()),
            next: None,
        }
    }

    pub fn split(
        &mut self,
        strategy: SplitStrategy,
    ) -> NodeRef<Self, Mutable> {
        let (left, right) = strategy.sizes(B);
        assert!(self.length == B);
        let mut new = NodeRef::<Self, _>::alloc();
        // SAFETY: Guaranteed by this type's invariants (length is always
        // accurate).
        unsafe {
            ptr::copy_nonoverlapping(
                (self.children.as_ptr() as *const T).wrapping_add(left),
                new.children.as_mut_ptr() as *mut T,
                right,
            );
        }
        new.next = self.next;
        self.next = Some(new.as_ptr());
        self.length = left;
        new.length = right;
        new
    }

    pub fn merge(&mut self, other: &mut Self) {
        let length = self.length;
        assert!(length <= B / 2);
        assert!(other.length <= B / 2);
        // SAFETY: Guaranteed by this type's invariants (length is always
        // accurate).
        unsafe {
            ptr::copy_nonoverlapping(
                other.children.as_ptr() as *const T,
                (self.children.as_mut_ptr() as *mut T).wrapping_add(length),
                other.length,
            );
        }
        assert!(self.next == Some(NonNull::from(&mut *other)));
        self.next = other.next;
        other.next = None;
        self.length += other.length;
        other.length = 0;
    }

    pub fn simple_insert(&mut self, i: usize, item: T) {
        let length = self.length;
        self.children[i..length + 1].rotate_right(1);
        self.children[i] = MaybeUninit::new(item);
        self.length += 1;
    }

    pub fn simple_remove(&mut self, i: usize) -> T {
        let length = self.length;
        assert!(length > 0);
        self.children[i..length].rotate_left(1);
        self.length -= 1;
        let item = mem::replace(
            &mut self.children[length - 1],
            MaybeUninit::uninit(),
        );
        // SAFETY: Items at 0..length are always initialized.
        unsafe { item.assume_init() }
    }

    pub fn child(&self, i: usize) -> &T {
        assert!(i < self.length);
        // SAFETY: Items at 0..length are always initialized. We can
        // dereference because we hand out references only according to
        // standard borrowing rules.
        unsafe { &*self.children[i].as_ptr() }
    }

    pub fn child_mut(&mut self, i: usize) -> &mut T {
        assert!(i < self.length);
        // SAFETY: Items at 0..length are always initialized. We can
        // dereference because we hand out references only according to
        // standard borrowing rules.
        unsafe { &mut *self.children[i].as_mut_ptr() }
    }

    pub fn take_raw_child(&mut self, i: usize) -> MaybeUninit<T> {
        self.length = self.length.min(i);
        mem::replace(&mut self.children[i], MaybeUninit::uninit())
    }

    pub fn size(&self) -> usize {
        self.length
    }
}

// SAFETY: `Node` may be implemented by `LeafNode`.
unsafe impl<T, const B: usize> Node for LeafNode<T, B> {
    type Prefix = Prefix<T, B>;
    type Child = T;

    unsafe fn new() -> Self {
        // SAFETY: Checked by caller.
        unsafe { Self::new() }
    }

    fn item_size(_item: &Self::Child) -> usize {
        1
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

impl<T, const B: usize, R: RefKind> NodeRef<LeafNode<T, B>, R> {
    pub fn into_child<'a>(self, i: usize) -> &'a T {
        // SAFETY: The underlying node's life is not tied to this `NodeRef`'s
        // life, so we can return a reference to data in the node with any
        // lifetime. In order for the underlying node to be dropped, a mutable
        // `NodeRef` would have to be created (one cannot exist right now
        // because `self` is an immutable `NodeRef`), which is an unsafe
        // operation that requires the caller to ensure that no references to
        // node data (such as those returned by this method) exist.
        unsafe { &*(self.child(i) as *const _) }
    }

    pub fn into_next(self) -> Result<Self, Self> {
        if let Some(node) = self.next {
            Ok(Self(node, Pd))
        } else {
            Err(self)
        }
    }
}

impl<T, const B: usize> NodeRef<LeafNode<T, B>, Mutable> {
    pub fn into_child_mut<'a>(mut self, i: usize) -> &'a mut T {
        // SAFETY: The underlying node's life is not tied to this `NodeRef`'s
        // life, so we can return a reference to data in the node with any
        // lifetime. In order for the underlying node to be dropped, a mutable
        // `NodeRef` would have to be created (one cannot exist right now
        // because `self` is already the one allowed mutable `NodeRef`), which
        // is an unsafe operation that requires the caller to ensure that no
        // references to node data (such as those returned by this method)
        // exist.
        unsafe { &mut *(self.child_mut(i) as *mut _) }
    }
}
