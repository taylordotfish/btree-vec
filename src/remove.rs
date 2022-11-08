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

use super::node::{InternalNode, Node, Prefix};
use super::node::{LeafRef, Mutable, NodeRef, PrefixRef};
use core::mem;

struct Removal<N> {
    node: NodeRef<N, Mutable>,
    kind: RemovalKind,
}

#[derive(Debug)]
enum RemovalKind {
    /// `src` was merged with `dest`. `src` and `dest` are the indices of two
    /// nodes within the same parent.
    Merged {
        src: usize,
        dest: usize,
    },
    /// An item in `src` was moved to `dest`. `src` and `dest` are the indices
    /// of two nodes within the same parent.
    Moved {
        src: usize,
        dest: usize,
        size: usize,
    },
    /// Removal doesn't need to be propagated anymore -- the item was removed
    /// from a node with more than the minimum number of children.
    Absorbed {
        index: usize,
    },
}

enum RemovalResult<N, T, const B: usize> {
    Removal(Removal<InternalNode<T, B>>),
    Done(NodeRef<N, Mutable>),
}

fn handle_removal<N, T, const B: usize>(
    removal: Removal<N>,
) -> RemovalResult<N, T, B>
where
    N: Node<Prefix = Prefix<T, B>>,
{
    let node = removal.node;
    let (parent, empty) = match removal.kind {
        RemovalKind::Merged {
            src,
            dest,
        } => {
            let mut parent = node.into_parent().ok().unwrap();
            let size = mem::replace(&mut parent.sizes[src], 0);
            parent.sizes[dest] += size;
            parent.sizes[dest] -= 1;
            (parent, Some(src))
        }
        RemovalKind::Moved {
            src,
            dest,
            size,
        } => {
            let mut parent = node.into_parent().ok().unwrap();
            parent.sizes[src] -= size;
            parent.sizes[dest] += size;
            parent.sizes[dest] -= 1;
            (parent, None)
        }
        RemovalKind::Absorbed {
            index,
        } => match node.into_parent() {
            Ok(mut parent) => {
                parent.sizes[index] -= 1;
                (parent, None)
            }
            Err(node) => return RemovalResult::Done(node),
        },
    };

    if let Some(empty) = empty {
        let (removal, child) = remove_once(parent, empty);
        child.0.destroy();
        RemovalResult::Removal(removal)
    } else {
        RemovalResult::Removal(Removal {
            kind: RemovalKind::Absorbed {
                index: parent.index(),
            },
            node: parent,
        })
    }
}

fn remove_once<N, T, const B: usize>(
    mut node: NodeRef<N, Mutable>,
    i: usize,
) -> (Removal<N>, N::Child)
where
    N: Node<Prefix = Prefix<T, B>>,
{
    let item = node.simple_remove(i);
    let make_result = move |kind, node| {
        (
            Removal {
                node,
                kind,
            },
            item,
        )
    };

    let (mut left, mid, mut right) = node.siblings_mut();
    let has_sibling = left.is_some() || right.is_some();
    if mid.length() >= B / 2 || !has_sibling {
        return make_result(
            RemovalKind::Absorbed {
                index: node.index(),
            },
            node,
        );
    }

    if let Some(left) = &mut left {
        if left.length() > B / 2 {
            let moved = left.simple_remove(left.length() - 1);
            let size = N::item_size(&moved);
            let kind = RemovalKind::Moved {
                src: left.index(),
                dest: mid.index(),
                size,
            };
            node.simple_insert(0, moved);
            return make_result(kind, node);
        }
    }

    if let Some(right) = &mut right {
        if right.length() > B / 2 {
            let moved = right.simple_remove(0);
            let size = N::item_size(&moved);
            let kind = RemovalKind::Moved {
                src: right.index(),
                dest: mid.index(),
                size,
            };
            node.simple_insert(node.length(), moved);
            make_result(kind, node)
        } else {
            mid.merge(right);
            make_result(
                RemovalKind::Merged {
                    src: right.index(),
                    dest: mid.index(),
                },
                node,
            )
        }
    } else if let Some(left) = &mut left {
        left.merge(mid);
        make_result(
            RemovalKind::Merged {
                src: mid.index(),
                dest: left.index(),
            },
            node,
        )
    } else {
        unreachable!();
    }
}

pub fn remove<T, const B: usize>(
    node: LeafRef<T, B, Mutable>,
    i: usize,
) -> (PrefixRef<T, B, Mutable>, T) {
    let (removal, item) = remove_once(node, i);
    let result = handle_removal(removal);
    let mut removal = match result {
        RemovalResult::Removal(removal) => removal,
        RemovalResult::Done(root) => return (root.into_prefix(), item),
    };
    loop {
        removal = match handle_removal(removal) {
            RemovalResult::Removal(removal) => removal,
            RemovalResult::Done(mut root) => {
                let root = if root.length() == 1 {
                    let child = root.simple_remove(0).0;
                    root.destroy();
                    child
                } else {
                    root.into_prefix()
                };
                return (root, item);
            }
        }
    }
}
