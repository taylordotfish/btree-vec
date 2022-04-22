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

use super::node::{InternalNode, Node, Prefix};
use super::node::{LeafRef, Mutable, NodeRef, PrefixRef};
use core::mem;

struct Removal<N> {
    node: NodeRef<N, Mutable>,
    kind: RemovalKind,
}

#[derive(Debug)]
enum RemovalKind {
    Merged {
        src: usize,
        dest: usize,
    },
    Moved {
        src: usize,
        dest: usize,
        size: usize,
    },
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
            let size = mem::replace(parent.child_mut(src).1, 0);
            let dest = parent.child_mut(dest);
            *dest.1 += size;
            *dest.1 -= 1;
            (parent, Some(src))
        }
        RemovalKind::Moved {
            src,
            dest,
            size,
        } => {
            let mut parent = node.into_parent().ok().unwrap();
            *parent.child_mut(src).1 -= size;
            let dest = parent.child_mut(dest);
            *dest.1 += size;
            *dest.1 -= 1;
            (parent, None)
        }
        RemovalKind::Absorbed {
            index,
        } => match node.into_parent() {
            Ok(mut parent) => {
                *parent.child_mut(index).1 -= 1;
                (parent, None)
            }
            Err(node) => {
                return RemovalResult::Done(node);
            }
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
            mid.simple_insert(0, moved);
            return make_result(
                RemovalKind::Moved {
                    src: left.index(),
                    dest: mid.index(),
                    size,
                },
                node,
            );
        }
    }

    if let Some(right) = &mut right {
        if right.length() > B / 2 {
            let moved = right.simple_remove(0);
            let size = N::item_size(&moved);
            mid.simple_insert(mid.length(), moved);
            make_result(
                RemovalKind::Moved {
                    src: right.index(),
                    dest: mid.index(),
                    size,
                },
                node,
            )
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
