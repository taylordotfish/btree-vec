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

use alloc::boxed::Box;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use tagged_pointer::TaggedPtr;

mod internal;
mod leaf;
mod parent_ptr;

pub use internal::InternalNode;
pub use leaf::LeafNode;
use parent_ptr::ParentPtr;

/// # Safety
///
/// May be implemented only by `InternalNode` and `LeafNode`. (These types
/// have specific behavior that certain uses may require -- this behavior may
/// be more specifically documented later.)
pub unsafe trait Node: Sized {
    type Prefix;
    type Child;

    /// # Safety
    ///
    /// For use only by [`ExclusiveRef::alloc`].
    unsafe fn new() -> Self;
    fn item_size(item: &Self::Child) -> usize;
    fn prefix(&self) -> &Self::Prefix;
    fn prefix_mut(&mut self) -> &mut Self::Prefix;
    fn size(&self) -> usize;
    fn length(&self) -> usize;
    fn index(&self) -> usize;
    fn simple_insert(&mut self, i: usize, item: Self::Child);
    fn simple_remove(&mut self, i: usize) -> Self::Child;
    fn split(&mut self) -> ExclusiveRef<Self>;
    fn merge(&mut self, other: &mut Self);
}

#[repr(C)]
pub struct Prefix<T, const B: usize> {
    parent: ParentPtr<T, B>,
    index: usize,
    phantom: PhantomData<NonNull<T>>,
}

impl<T, const B: usize> Prefix<T, B> {
    fn new(is_leaf: bool) -> Self {
        Self {
            parent: ParentPtr::new(is_leaf),
            index: 0,
            phantom: PhantomData,
        }
    }
}

pub type PrefixPtr<T, const B: usize> = NonNull<Prefix<T, B>>;
pub struct ExclusiveRef<N>(NonNull<N>);

pub type ExclusiveLeaf<T, const B: usize> = ExclusiveRef<LeafNode<T, B>>;
pub type ExclusiveInternal<T, const B: usize> =
    ExclusiveRef<InternalNode<T, B>>;
pub type ExclusivePrefix<T, const B: usize> = ExclusiveRef<Prefix<T, B>>;

impl<N> ExclusiveRef<N> {
    /// # Safety
    ///
    /// * There must not be any other [`ExclusiveRef`]s being used for
    ///   mutation.
    /// * If this [`ExclusiveRef`] will be used for mutation, there must not be
    ///   any other [`ExclusiveRef`]s.
    ///
    /// Note that if this `ExclusiveRef` will *not* be used for mutation,
    /// any method that mutates data through the `ExclusiveRef` *must not*
    /// be called.
    pub unsafe fn new(ptr: NonNull<N>) -> Self {
        Self(ptr)
    }

    pub fn as_ptr(&self) -> NonNull<N> {
        self.0
    }
}

impl<N: Node> ExclusiveRef<N> {
    pub fn alloc() -> Self {
        Self(unsafe {
            NonNull::new_unchecked(Box::into_raw(Box::new(N::new())))
        })
    }
}

impl<N> Deref for ExclusiveRef<N> {
    type Target = N;

    fn deref(&self) -> &N {
        // SAFETY: `ExclusiveRef` is designed to make this safe.
        unsafe { self.0.as_ref() }
    }
}

impl<N> DerefMut for ExclusiveRef<N> {
    fn deref_mut(&mut self) -> &mut N {
        // SAFETY: `ExclusiveRef` is designed to make this safe.
        unsafe { self.0.as_mut() }
    }
}

pub enum ExclusiveCast<T, const B: usize> {
    Internal(ExclusiveRef<InternalNode<T, B>>),
    Leaf(ExclusiveRef<LeafNode<T, B>>),
}

impl<T, const B: usize> ExclusiveRef<Prefix<T, B>> {
    pub fn cast(self) -> ExclusiveCast<T, B> {
        if self.parent.is_leaf() {
            ExclusiveCast::Leaf(ExclusiveRef(self.0.cast()))
        } else {
            ExclusiveCast::Internal(ExclusiveRef(self.0.cast()))
        }
    }

    pub fn destroy(self) {
        match self.cast() {
            ExclusiveCast::Internal(node) => node.destroy(),
            ExclusiveCast::Leaf(node) => node.destroy(),
        }
    }
}

impl<N, T, const B: usize> ExclusiveRef<N>
where
    N: Node<Prefix = Prefix<T, B>>,
{
    pub fn destroy(mut self) {
        assert!(self.parent().is_none());
        // SAFETY: `ExclusiveRef` is designed to make this safe.
        unsafe { Box::from_raw(self.0.as_ptr()) };
    }

    pub fn into_prefix(mut self) -> ExclusivePrefix<T, B> {
        ExclusiveRef(NonNull::from(self.prefix_mut()))
    }

    pub fn into_parent(
        self,
    ) -> Result<ExclusiveRef<InternalNode<T, B>>, Self> {
        if let Some(p) = self.prefix().parent.get() {
            Ok(ExclusiveRef(p))
        } else {
            Err(self)
        }
    }

    pub fn parent(&mut self) -> Option<&mut InternalNode<T, B>> {
        // SAFETY: `ExclusiveRef` is designed to make this safe.
        self.prefix().parent.get().map(|mut p| unsafe { p.as_mut() })
    }

    pub fn siblings(&mut self) -> (Option<&mut N>, &mut N, Option<&mut N>) {
        let index = self.index();
        let parent = if let Some(parent) = self.parent() {
            parent
        } else {
            return (None, self, None);
        };
        // SAFETY: We can borrow multiple different children simultaneously.
        // (We aren't creating `ExclusiveRef`s out of them.)
        let [left, right] = [index.checked_sub(1), Some(index + 1)].map(|i| {
            i.and_then(|i| parent.try_child_mut(i))
                .map(|p| unsafe { NonNull::from(p.0).cast().as_mut() })
        });
        (left, self, right)
    }
}
