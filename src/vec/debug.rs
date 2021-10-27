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

use super::node::{ExclusiveCast, ExclusiveInternal, ExclusivePrefix, Node};
use super::BTreeVec;
use core::fmt::Debug;

impl<T: Debug, const B: usize> BTreeVec<T, B> {
    pub(crate) fn debug(&self) {
        eprintln!("tree size: {}", self.size);
        // SAFETY: This `ExclusiveRef` will not be used for mutation, and we
        // create `ExclusiveRef`s only according to standard borrow rules, so
        // no mutating `ExclusiveRef`s exist.
        debug(unsafe { ExclusivePrefix::new(self.root.unwrap()) }, 0, 0);
    }
}

fn print_indent(indent: usize) {
    for _ in 0..indent {
        eprint!("  ");
    }
}

macro_rules! debug_print {
    ($indent:expr, $($args:tt)*) => {
        print_indent($indent);
        eprintln!($($args)*);
    };
}

fn debug<T: Debug, const B: usize>(
    root: ExclusivePrefix<T, B>,
    depth: usize,
    indent: usize,
) -> Option<ExclusiveInternal<T, B>> {
    match root.cast() {
        ExclusiveCast::Leaf(node) => {
            debug_print!(indent, "depth {}: leaf", depth);
            debug_print!(indent, "length: {}", node.length());
            debug_print!(indent, "index: {}", node.index());
            print_indent(indent);
            for i in 0..node.length() {
                eprint!("{:?}, ", node.child(i))
            }
            eprintln!();
            node.into_parent().ok()
        }

        ExclusiveCast::Internal(mut node) => {
            debug_print!(indent, "depth {}: internal", depth);
            debug_print!(indent, "length: {}", node.length());
            debug_print!(indent, "index: {}", node.index());
            for i in 0..node.length() {
                eprintln!();
                debug_print!(indent + 1, "child {}", i);
                debug_print!(indent + 1, "size: {}", node.sizes()[i]);
                node =
                    debug(node.into_child(i), depth + 1, indent + 1).unwrap();
            }
            node.into_parent().ok()
        }
    }
}
