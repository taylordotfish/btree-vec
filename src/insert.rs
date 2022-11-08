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

use super::node::{InternalNode, Node, NodeRef, Prefix, SplitStrategy};
use super::node::{InternalRef, LeafRef, Mutable, PrefixRef};

struct Insertion<N> {
    node: NodeRef<N, Mutable>,
    /// The new node created as a result of splitting `node` (in response to
    /// an attempt to insert into a full node), or [`None`] if `node` wasn't
    /// split.
    new: Option<NodeRef<N, Mutable>>,
}

enum InsertionResult<T, const B: usize> {
    Insertion(Insertion<InternalNode<T, B>>),
    Done(PrefixRef<T, B, Mutable>),
}

fn handle_insertion<N, T, const B: usize>(
    insertion: Insertion<N>,
    root_size: usize,
) -> InsertionResult<T, B>
where
    N: Node<Prefix = Prefix<T, B>>,
{
    let index = insertion.node.index();
    let new = insertion.new.map(|new| {
        let size = new.size();
        (new, size)
    });

    let mut parent = match insertion.node.into_parent() {
        Ok(parent) => parent,
        Err(root) => {
            if new.is_none() {
                return InsertionResult::Done(root.into_prefix());
            }
            // New root
            let mut parent = InternalRef::alloc();
            parent.simple_insert(0, (root.into_prefix(), root_size));
            parent
        }
    };

    parent.sizes[index] += 1;
    let (new, new_size) = if let Some(new @ (_, size)) = new {
        parent.sizes[index] -= size;
        new
    } else {
        return InsertionResult::Insertion(Insertion {
            node: parent,
            new: None,
        });
    };

    let new = (new.into_prefix(), new_size);
    let split = insert_once(&mut parent, index + 1, new);
    InsertionResult::Insertion(Insertion {
        node: parent,
        new: split,
    })
}

/// If `node` is full, splits `node` and returns the new node. Otherwise,
/// returns [`None`].
fn insert_once<N, T, const B: usize>(
    node: &mut NodeRef<N, Mutable>,
    i: usize,
    item: N::Child,
) -> Option<NodeRef<N, Mutable>>
where
    N: Node<Prefix = Prefix<T, B>>,
{
    let mut split = None;
    if node.length() == B {
        if let Some(i) = i.checked_sub(B - B / 2) {
            split
                .insert(node.split(SplitStrategy::LargerLeft))
                .simple_insert(i, item);
            return split;
        }
        split = Some(node.split(SplitStrategy::LargerRight));
    }
    node.simple_insert(i, item);
    split
}

pub fn insert<T, const B: usize>(
    mut node: LeafRef<T, B, Mutable>,
    i: usize,
    item: T,
    root_size: usize,
) -> PrefixRef<T, B, Mutable> {
    let mut result = handle_insertion(
        Insertion {
            new: insert_once(&mut node, i, item),
            node,
        },
        root_size,
    );
    loop {
        result = match result {
            InsertionResult::Done(root) => return root,
            InsertionResult::Insertion(ins) => {
                handle_insertion(ins, root_size)
            }
        }
    }
}
