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

use btree_vec::BTreeVec;

#[test]
fn basic_remove() {
    let mut vec = BTreeVec::<u8, 7>::create();
    for i in 0..8 {
        vec.push(i);
    }
    vec.remove(7);
    vec.remove(5);
    vec.remove(3);
    vec.remove(1);
    assert!(vec.len() == 4);
    for i in 0..4 {
        assert!(vec[i as usize] == i * 2);
    }
}

#[test]
fn medium_remove() {
    let mut vec = BTreeVec::<u8, 8>::create();
    for i in 0..32 {
        vec.push(i);
    }
    for i in 0..16 {
        vec.remove(i);
    }
    assert!(vec.len() == 16);
    for i in 0..16 {
        assert!(vec[i as usize] == i * 2 + 1);
    }
}

#[test]
fn large_remove() {
    let mut vec = BTreeVec::<u8, 7>::create();
    for i in 0..128 {
        vec.push(i);
    }
    for i in 0..64 {
        vec.remove(i);
    }
    assert!(vec.len() == 64);
    for i in 0..64 {
        assert!(vec[i as usize] == i * 2 + 1);
    }
}

#[test]
fn large_remove_front() {
    let mut vec = BTreeVec::<u8, 7>::create();
    for i in 0..128 {
        vec.push(i);
    }
    for _ in 0..64 {
        vec.remove(0);
    }
    assert!(vec.len() == 64);
    for i in 0..64 {
        assert!(vec[i as usize] == i + 64);
    }
}

#[test]
fn large_pop() {
    let mut vec = BTreeVec::<u8, 8>::create();
    for i in 0..128 {
        vec.push(i);
    }
    for _ in 0..64 {
        vec.pop();
    }
    assert!(vec.len() == 64);
    for i in 0..64 {
        assert!(vec[i as usize] == i);
    }
}
