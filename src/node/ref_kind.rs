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

use super::{InternalRef, Node, NodeRef, Prefix, PrefixRef};
use core::marker::PhantomData as Pd;
use core::ptr::NonNull;

pub struct Mutable(());
pub struct Immutable(());

mod sealed {
    pub trait Sealed {}
}

pub trait RefKind: sealed::Sealed + Sized {
    fn into_prefix<N, T, const B: usize>(
        r: NodeRef<N, Self>,
    ) -> PrefixRef<T, B, Self>
    where
        N: Node<Prefix = Prefix<T, B>>;

    fn into_child<T, const B: usize>(
        r: InternalRef<T, B, Self>,
        i: usize,
    ) -> PrefixRef<T, B, Self>;
}

impl sealed::Sealed for Mutable {}
impl sealed::Sealed for Immutable {}

impl RefKind for Mutable {
    fn into_prefix<N, T, const B: usize>(
        mut r: NodeRef<N, Self>,
    ) -> PrefixRef<T, B, Self>
    where
        N: Node<Prefix = Prefix<T, B>>,
    {
        NodeRef(NonNull::from(r.prefix_mut()), Pd)
    }

    fn into_child<T, const B: usize>(
        mut r: InternalRef<T, B, Self>,
        i: usize,
    ) -> PrefixRef<T, B, Self> {
        NodeRef(NonNull::from(r.child_mut(i).0), Pd)
    }
}

impl RefKind for Immutable {
    fn into_prefix<N, T, const B: usize>(
        r: NodeRef<N, Self>,
    ) -> PrefixRef<T, B, Self>
    where
        N: Node<Prefix = Prefix<T, B>>,
    {
        NodeRef(NonNull::from(r.prefix()), Pd)
    }

    fn into_child<T, const B: usize>(
        r: InternalRef<T, B, Self>,
        i: usize,
    ) -> PrefixRef<T, B, Self> {
        r.child_ref(i)
    }
}
