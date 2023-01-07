/*
 * Copyright (C) 2021-2023 taylor.fish <contact@taylor.fish>
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

use super::node::{InternalRef, LeafRef, NodeRef, PrefixRef};
use super::node::{Node, PrefixCast};
use super::BTreeVec;
use alloc::collections::BTreeMap;
use core::cell::RefCell;
use core::fmt::{self, Debug, Display, Formatter};
use core::ptr::NonNull;

// Indent for use in format strings
const I1: &str = "    ";

struct IdMap<T>(BTreeMap<T, usize>);

impl<T> IdMap<T> {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }
}

impl<T: Ord> IdMap<T> {
    pub fn get(&mut self, value: T) -> usize {
        let len = self.0.len();
        *self.0.entry(value).or_insert(len + 1)
    }
}

pub struct State(IdMap<NonNull<u8>>);

impl State {
    pub fn new() -> Self {
        Self(IdMap::new())
    }

    fn id<T, const B: usize>(
        &mut self,
        node: impl Into<PrefixRef<T, B>>,
    ) -> usize {
        self.0.get(node.into().as_ptr().cast())
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Debug, const B: usize> BTreeVec<T, B> {
    pub fn debug<'a>(&'a self, state: &'a mut State) -> VecDebug<'a, T, B> {
        VecDebug {
            state: RefCell::new(state),
            vec: self,
        }
    }
}

#[must_use]
pub struct VecDebug<'a, T, const B: usize> {
    state: RefCell<&'a mut State>,
    vec: &'a BTreeVec<T, B>,
}

impl<'a, T: Debug, const B: usize> Display for VecDebug<'a, T, B> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut state = self.state.borrow_mut();
        writeln!(f, "digraph {{")?;
        writeln!(
            f,
            "{I1}R [label=\"Size: {}\" shape=rectangle]",
            self.vec.len(),
        )?;
        if let Some(root) = self.vec.root {
            // SAFETY: We create `NodeRef`s only according to standard borrow
            // rules, so no mutable references to data exist.
            let root = unsafe { NodeRef::new(root) };
            writeln!(f, "{I1}R -> N{}", state.id(root))?;
            fmt_prefix(&mut state, f, root)?;
        }
        writeln!(f, "}}")
    }
}

fn fmt_prefix<T: Debug, const B: usize>(
    state: &mut State,
    f: &mut Formatter<'_>,
    node: PrefixRef<T, B>,
) -> fmt::Result {
    match node.cast() {
        PrefixCast::Internal(node) => fmt_internal(state, f, node),
        PrefixCast::Leaf(node) => fmt_leaf(state, f, node),
    }
}

fn fmt_internal<T: Debug, const B: usize>(
    state: &mut State,
    f: &mut Formatter<'_>,
    node: InternalRef<T, B>,
) -> fmt::Result {
    let id = state.id(node);
    writeln!(
        f,
        "{I1}N{id} [label=\"i{id}\\n#{}\\nL: {}\" shape=rectangle]",
        node.index(),
        node.length(),
    )?;
    for i in 0..node.length() {
        let child = node.child_ref(i);
        let child_id = state.id(child);
        writeln!(f, "{I1}N{id} -> N{child_id} [label={}]", node.sizes[i])?;
        fmt_prefix(state, f, child)?;
    }
    Ok(())
}

fn fmt_leaf<T: Debug, const B: usize>(
    state: &mut State,
    f: &mut Formatter<'_>,
    node: LeafRef<T, B>,
) -> fmt::Result {
    let id = state.id(node);
    writeln!(
        f,
        "{I1}N{id} [label=\"L{id}\\n#{}\\nL: {}\" shape=rectangle]",
        node.index(),
        node.length(),
    )?;
    for i in 0..node.length() {
        writeln!(f, "{I1}N{id} -> N{id}C{i}")?;
        writeln!(
            f,
            "{I1}N{id}C{i} [label=\"{:?}\" shape=rectangle]",
            node.child(i),
        )?;
    }
    Ok(())
}
