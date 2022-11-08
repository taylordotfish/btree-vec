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

use super::node::{ExclusiveInternal, ExclusiveLeaf, ExclusivePrefix};
use super::node::{ExclusiveRef, InternalNode, Node, Prefix};

struct Insertion<N> {
    node: ExclusiveRef<N>,
    new: Option<ExclusiveRef<N>>,
}

enum InsertionResult<T, const B: usize> {
    Insertion(Insertion<InternalNode<T, B>>),
    Done(ExclusivePrefix<T, B>),
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
            let mut parent = ExclusiveInternal::alloc();
            parent.simple_insert(0, (root.into_prefix(), root_size));
            parent
        }
    };

    let child = parent.child_mut(index);
    *child.1 += 1;
    let (new, new_size) = if let Some(new @ (_, size)) = new {
        *child.1 -= size;
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

fn insert_once<N, T, const B: usize>(
    node: &mut ExclusiveRef<N>,
    i: usize,
    item: N::Child,
) -> Option<ExclusiveRef<N>>
where
    N: Node<Prefix = Prefix<T, B>>,
{
    let mut split = None;
    if node.length() == B {
        let new = split.insert(node.split());
        if let Some(i) = i.checked_sub(B - B / 2) {
            new.simple_insert(i, item);
            return split;
        }
    }
    node.simple_insert(i, item);
    split
}

pub fn insert<T, const B: usize>(
    mut node: ExclusiveLeaf<T, B>,
    i: usize,
    item: T,
    root_size: usize,
) -> ExclusivePrefix<T, B> {
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
