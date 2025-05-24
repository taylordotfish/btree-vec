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

use crate::{Allocator, VerifiedAlloc};
use alloc::alloc::{Layout, handle_alloc_error};
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

use PhantomData as Pd;

mod internal;
mod leaf;
mod parent_ptr;

pub use internal::InternalNode;
pub use leaf::LeafNode;
use parent_ptr::ParentPtr;

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

mod sealed {
    pub trait Sealed {}
    impl<T, const B: usize> Sealed for super::InternalNode<T, B> {}
    impl<T, const B: usize> Sealed for super::LeafNode<T, B> {}
}

pub trait Node: sealed::Sealed + Sized {
    type Prefix;
    type Child;

    fn new(_: node_ref_alloc::Token) -> Self;
    fn item_size(item: &Self::Child) -> usize;
    fn prefix(&self) -> &Self::Prefix;
    fn size(&self) -> usize;
    fn length(&self) -> usize;
    fn index(&self) -> usize;
    fn simple_insert(
        this: &mut NodeRef<Self, Mutable>,
        i: usize,
        item: Self::Child,
    );
    fn simple_remove(&mut self, i: usize) -> Self::Child;
    fn split(
        &mut self,
        strategy: SplitStrategy,
        alloc: &VerifiedAlloc<impl Allocator>,
    ) -> NodeRef<Self, Mutable>;
    fn merge(&mut self, other: &mut Self);
    fn destroy_children(&mut self, alloc: &VerifiedAlloc<impl Allocator>) {
        let _ = alloc;
    }
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

pub struct Mutable(());
pub struct Immutable(());

/// `N` is the node type; it should be [`InternalNode`] or [`LeafNode`].
/// `R` is the reference kind; it should be [`Immutable`] or [`Mutable`].
pub struct NodeRef<N, R = Immutable>(NonNull<N>, PhantomData<fn() -> R>);

pub type LeafRef<T, const B: usize, R = Immutable> =
    NodeRef<LeafNode<T, B>, R>;

pub type InternalRef<T, const B: usize, R = Immutable> =
    NodeRef<InternalNode<T, B>, R>;

pub type PrefixRef<T, const B: usize, R = Immutable> =
    NodeRef<Prefix<T, B>, R>;

impl<N, R> NodeRef<N, R> {
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

mod node_ref_alloc {
    use super::*;

    /// Ensures that only [`alloc`](self::alloc) can create nodes.
    pub struct Token(());

    pub fn alloc<N: Node>(
        alloc: &VerifiedAlloc<impl Allocator>,
    ) -> NodeRef<N, Mutable> {
        let layout = Layout::new::<N>();
        let ptr = alloc
            .allocate(layout)
            .unwrap_or_else(|_| handle_alloc_error(layout))
            .cast::<N>();
        unsafe {
            ptr.as_ptr().write(N::new(Token(())));
        }
        NodeRef(ptr, Pd)
    }
}

impl<N: Node> NodeRef<N, Mutable> {
    pub fn alloc(alloc: &VerifiedAlloc<impl Allocator>) -> Self {
        node_ref_alloc::alloc(alloc)
    }

    pub fn simple_insert(&mut self, i: usize, item: N::Child) {
        N::simple_insert(self, i, item);
    }
}

pub enum PrefixCast<T, const B: usize, R> {
    Internal(NodeRef<InternalNode<T, B>, R>),
    Leaf(NodeRef<LeafNode<T, B>, R>),
}

impl<T, const B: usize, R> NodeRef<Prefix<T, B>, R> {
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
    pub fn destroy(self, alloc: &VerifiedAlloc<impl Allocator>) {
        match self.cast() {
            PrefixCast::Internal(node) => node.destroy(alloc),
            PrefixCast::Leaf(node) => node.destroy(alloc),
        }
    }
}

impl<N, T, const B: usize> NodeRef<N, Mutable>
where
    N: Node<Prefix = Prefix<T, B>>,
{
    pub fn destroy(mut self, alloc: &VerifiedAlloc<impl Allocator>) {
        assert!(self.parent().is_none());
        self.destroy_children(alloc);
        // SAFETY: `self.0` is always an initialized, properly aligned pointer.
        let layout = Layout::for_value(&unsafe { self.0.as_ptr().read() });
        // SAFETY: Guaranteed by `VerifiedAlloc`.
        unsafe {
            alloc.deallocate(self.0.cast(), layout);
        }
    }
}

impl<N, T, const B: usize, R> NodeRef<N, R>
where
    N: Node<Prefix = Prefix<T, B>>,
{
    pub fn into_prefix(self) -> PrefixRef<T, B, R> {
        NodeRef(self.0.cast(), Pd)
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

        let siblings = [index.checked_sub(1), index.checked_add(1)];
        let [left, right] = siblings.map(|i| {
            i.and_then(|i| parent.child_ptr(i)).map(|p| {
                // SAFETY: We can borrow multiple different children
                // simultaneously. (We aren't creating `NodeRef`s out of them.)
                unsafe { p.cast().as_mut() }
            })
        });
        (left, self, right)
    }
}

impl<N> Clone for NodeRef<N> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<N> Copy for NodeRef<N> {}

impl<N, R> Deref for NodeRef<N, R> {
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

impl<N, T, const B: usize, R> From<NodeRef<N, R>> for PrefixRef<T, B, R>
where
    N: Node<Prefix = Prefix<T, B>>,
{
    fn from(r: NodeRef<N, R>) -> Self {
        Self(r.0.cast(), Pd)
    }
}
