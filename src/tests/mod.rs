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

use crate::debug;
use crate::BTreeVec;
use core::fmt::Debug;
use std::fs::File;
use std::io::{self, Write};
use std::process::Command;

mod insert;
mod remove;

#[test]
fn basic_iter() {
    let mut vec = BTreeVec::<u8, 7>::create();
    for i in 0..8 {
        vec.push(i);
    }
    assert!(vec.iter().copied().eq(0..8));
}

#[test]
fn medium_iter() {
    let mut vec = BTreeVec::<_, 7>::create();
    for i in 0..32_u32 {
        vec.push(Box::new(i));
    }
    assert!(vec.into_iter().map(|b| *b).eq(0..32));
}

#[test]
fn large_iter() {
    let mut vec = BTreeVec::<u8, 7>::create();
    for i in 0..128 {
        vec.push(i);
    }
    for i in 0..16 {
        assert!(vec.pop() == Some(128 - i - 1));
    }
    for i in 0..16 {
        assert!(vec.remove(0) == i);
    }
    assert!(vec.iter().copied().eq(16..112));
}

#[test]
fn small_b() {
    let mut vec = BTreeVec::<u8, 4>::create();
    for i in 0..32 {
        vec.push(i);
    }
    for _ in 0..16 {
        vec.pop();
    }
    assert!((0..16).eq(vec));
}

#[test]
fn smallest_b() {
    let mut vec = BTreeVec::<u8, 4>::create();
    for i in 0..24 {
        vec.push(i);
    }
    for _ in 0..12 {
        vec.remove(7);
    }
    assert!(vec.iter().copied().eq((0..7).chain(19..24)));
}

#[test]
#[should_panic]
fn too_small_b() {
    BTreeVec::<u8, 2>::create();
}

#[cfg(feature = "dropck_eyepatch")]
#[test]
fn same_life_ref() {
    let n = Box::new(123);
    let mut vec = BTreeVec::<_, 16>::create();
    vec.push(&n);
    drop(n);
}

#[allow(dead_code)]
fn make_graph<T: Debug, const B: usize>(
    vec: &BTreeVec<T, B>,
    state: &mut debug::State,
) -> io::Result<()> {
    let mut file = File::create("graph.dot")?;
    write!(file, "{}", vec.debug(state))?;
    file.sync_all()?;
    drop(file);
    Command::new("dot")
        .arg("-Tpng")
        .arg("-ograph.png")
        .arg("graph.dot")
        .status()?;
    Ok(())
}
