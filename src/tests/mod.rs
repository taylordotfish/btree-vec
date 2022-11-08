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

use crate::vec::BTreeVec;

mod insert;
mod remove;

#[test]
fn basic_iter() {
    let mut vec = BTreeVec::<u8, 7>::new();
    for i in 0..8 {
        vec.push(i);
    }
    for (i, n) in vec.iter().enumerate() {
        assert!(i == *n as usize);
    }
}

#[test]
fn medium_iter() {
    let mut vec = BTreeVec::<_, 7>::new();
    for i in 0..32_u32 {
        vec.push(Box::new(i));
    }
    assert!(vec.into_iter().map(|b| *b).eq(0..32));
}

#[test]
fn large_iter() {
    let mut vec = BTreeVec::<u8, 7>::new();
    for i in 0..128 {
        vec.push(i);
    }
    for i in 0..16 {
        assert!(vec.pop() == Some(128 - i - 1));
    }
    for i in 0..16 {
        assert!(vec.remove(0) == i);
    }
    for (i, n) in vec.iter().enumerate() {
        assert!(i == *n as usize - 16);
    }
    vec.debug();
}

#[test]
fn small_b() {
    let mut vec = BTreeVec::<u8, 4>::new();
    for i in 0..32 {
        vec.push(i);
    }
    for _ in 0..16 {
        vec.pop();
    }
    for (i, n) in vec.iter().enumerate() {
        assert!(i == *n as usize);
    }
}

#[test]
#[should_panic]
fn too_small_b() {
    BTreeVec::<u8, 3>::new();
}
