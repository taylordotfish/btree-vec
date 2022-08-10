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

use alloc::boxed::Box;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

use PhantomData as Pd;

mod internal;
mod leaf;
mod parent_ptr;
mod ref_kind;

pub use internal::InternalNode;
pub use leaf::LeafNode;
use parent_ptr::ParentPtr;
pub use ref_kind::{Immutable, Mutable, RefKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum NodeKind {
    Internal = 0,
    Leaf = 1,
}

impl NodeKind {
    pub const VARIANTS: [Self; 2] = [Self::Internal, Self::Leaf];
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum SplitStrategy {
    LargerLeft,
    LargerRight,
}

impl SplitStrategy {
    /// Returns `(left, right)`. `b` is the B-tree branching factor.
    pub const fn sizes(self, b: usize) -> (usize, usize) {
        match self {
            SplitStrategy::LargerLeft => (b - b / 2, b / 2),
            SplitStrategy::LargerRight => (b / 2, b - b / 2),
        }
    }
}

/// # Safety
///
/// May be implemented only by [`InternalNode`] and [`LeafNode`]. (These types
/// have specific behavior that certain uses may require -- this behavior may
/// be more specifically documented later.)
pub unsafe trait Node: Sized {
    type Prefix;
    type Child;

    /// # Safety
    ///
    /// For use only by [`NodeRef::alloc`].
    unsafe fn new() -> Self;
    fn item_size(item: &Self::Child) -> usize;
    fn prefix(&self) -> &Self::Prefix;
    fn prefix_mut(&mut self) -> &mut Self::Prefix;
    fn size(&self) -> usize;
    fn length(&self) -> usize;
    fn index(&self) -> usize;
    fn simple_insert(&mut self, i: usize, item: Self::Child);
    fn simple_remove(&mut self, i: usize) -> Self::Child;
    fn split(&mut self, strategy: SplitStrategy) -> NodeRef<Self, Mutable>;
    fn merge(&mut self, other: &mut Self);
}

#[repr(C)]
pub struct Prefix<T, const B: usize> {
    parent: ParentPtr<T, B>,
    index: usize,
    phantom: PhantomData<NonNull<T>>,
}

pub type PrefixPtr<T, const B: usize> = NonNull<Prefix<T, B>>;

impl<T, const B: usize> Prefix<T, B> {
    fn new(kind: NodeKind) -> Self {
        Self {
            parent: ParentPtr::new(kind),
            index: 0,
            phantom: PhantomData,
        }
    }
}

pub struct NodeRef<N, R = Immutable>(NonNull<N>, PhantomData<*const R>);

pub type LeafRef<T, const B: usize, R = Immutable> =
    NodeRef<LeafNode<T, B>, R>;

pub type InternalRef<T, const B: usize, R = Immutable> =
    NodeRef<InternalNode<T, B>, R>;

pub type PrefixRef<T, const B: usize, R = Immutable> =
    NodeRef<Prefix<T, B>, R>;

impl<N, R: RefKind> NodeRef<N, R> {
    pub fn as_ptr(&self) -> NonNull<N> {
        self.0
    }
}

impl<N> NodeRef<N> {
    /// # Safety
    ///
    /// * `ptr` must point to a valid, aligned object of type `N`.
    /// * There must not be any mutable references, including other
    ///   [`NodeRef`]s where `R` is [`Mutable`], to any data accessible via the
    ///   returned [`NodeRef`].
    pub unsafe fn new(ptr: NonNull<N>) -> Self {
        Self(ptr, Pd)
    }
}

impl<N> NodeRef<N, Mutable> {
    /// # Safety
    ///
    /// * `ptr` must point to a valid, aligned object of type `N`.
    /// * There must be no other references, including [`NodeRef`]s, to any
    ///   data accessible via the returned [`NodeRef`].
    pub unsafe fn new_mutable(ptr: NonNull<N>) -> Self {
        Self(ptr, Pd)
    }
}

impl<N: Node> NodeRef<N, Mutable> {
    pub fn alloc() -> Self {
        Self(
            // SAFETY: By definition, it is safe for `NodeRef::alloc` (and no
            // other function) to call `Node::new`.
            unsafe {
                NonNull::new_unchecked(Box::into_raw(Box::new(N::new())))
            },
            Pd,
        )
    }
}

pub enum PrefixCast<T, const B: usize, R> {
    Internal(NodeRef<InternalNode<T, B>, R>),
    Leaf(NodeRef<LeafNode<T, B>, R>),
}

impl<T, const B: usize, R: RefKind> NodeRef<Prefix<T, B>, R> {
    pub fn cast(self) -> PrefixCast<T, B, R> {
        match self.parent.kind() {
            NodeKind::Leaf => PrefixCast::Leaf(NodeRef(self.0.cast(), Pd)),
            NodeKind::Internal => {
                PrefixCast::Internal(NodeRef(self.0.cast(), Pd))
            }
        }
    }
}

impl<T, const B: usize> NodeRef<Prefix<T, B>, Mutable> {
    pub fn destroy(self) {
        match self.cast() {
            PrefixCast::Internal(node) => node.destroy(),
            PrefixCast::Leaf(node) => node.destroy(),
        }
    }
}

impl<N, T, const B: usize> NodeRef<N, Mutable>
where
    N: Node<Prefix = Prefix<T, B>>,
{
    pub fn destroy(self) {
        assert!(self.parent().is_none());
        // SAFETY: `NodeRef` is designed to make this safe.
        unsafe { Box::from_raw(self.0.as_ptr()) };
    }
}

impl<N, T, const B: usize, R: RefKind> NodeRef<N, R>
where
    N: Node<Prefix = Prefix<T, B>>,
{
    pub fn into_prefix(self) -> PrefixRef<T, B, R> {
        R::into_prefix(self)
    }

    pub fn into_parent(self) -> Result<NodeRef<InternalNode<T, B>, R>, Self> {
        if let Some(p) = self.prefix().parent.get() {
            Ok(NodeRef(p, Pd))
        } else {
            Err(self)
        }
    }

    pub fn parent(&self) -> Option<&InternalNode<T, B>> {
        // SAFETY: `NodeRef` is designed to make this safe.
        self.prefix().parent.get().map(|p| unsafe { p.as_ref() })
    }
}

impl<N, T, const B: usize> NodeRef<N>
where
    N: Node<Prefix = Prefix<T, B>>,
{
    #[allow(dead_code)]
    pub fn parent_ref(&self) -> Option<InternalRef<T, B>> {
        self.prefix().parent.get().map(|p| NodeRef(p, Pd))
    }
}

impl<N, T, const B: usize> NodeRef<N, Mutable>
where
    N: Node<Prefix = Prefix<T, B>>,
{
    pub fn parent_mut(&mut self) -> Option<&mut InternalNode<T, B>> {
        // SAFETY: `NodeRef` is designed to make this safe.
        self.prefix().parent.get().map(|mut p| unsafe { p.as_mut() })
    }

    pub fn siblings_mut(
        &mut self,
    ) -> (Option<&mut N>, &mut N, Option<&mut N>) {
        let index = self.index();
        let parent = if let Some(parent) = self.parent_mut() {
            parent
        } else {
            return (None, self, None);
        };
        // SAFETY: We can borrow multiple different children simultaneously.
        // (We aren't creating `NodeRef`s out of them.)
        let [left, right] = [index.checked_sub(1), Some(index + 1)].map(|i| {
            i.and_then(|i| parent.try_child_mut(i))
                .map(|p| unsafe { NonNull::from(p.0).cast().as_mut() })
        });
        (left, self, right)
    }
}

impl<N> Clone for NodeRef<N> {
    fn clone(&self) -> Self {
        Self(self.0, self.1)
    }
}

impl<N> Copy for NodeRef<N> {}

impl<N, R: RefKind> Deref for NodeRef<N, R> {
    type Target = N;

    fn deref(&self) -> &N {
        // SAFETY: `NodeRef` is designed to make this safe.
        unsafe { self.0.as_ref() }
    }
}

impl<N> DerefMut for NodeRef<N, Mutable> {
    fn deref_mut(&mut self) -> &mut N {
        // SAFETY: `NodeRef` is designed to make this safe.
        unsafe { self.0.as_mut() }
    }
}

impl<N> From<NodeRef<N, Mutable>> for NodeRef<N> {
    fn from(r: NodeRef<N, Mutable>) -> Self {
        Self(r.0, Pd)
    }
}

impl<N, T, const B: usize, R: RefKind> From<NodeRef<N, R>>
    for PrefixRef<T, B, R>
where
    N: Node<Prefix = Prefix<T, B>>,
{
    fn from(r: NodeRef<N, R>) -> Self {
        Self(r.0.cast(), Pd)
    }
}
