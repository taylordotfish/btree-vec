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

use super::BTreeVec;

#[test]
fn basic_push() {
    let mut vec = BTreeVec::<u8, 7>::new();
    for i in 0..8 {
        vec.push(i);
    }
    assert!(vec.len() == 8);
    for i in 0..8 {
        assert!(vec[i as usize] == i);
    }
}

#[test]
fn medium_push() {
    let mut vec = BTreeVec::<u8, 7>::new();
    for i in 0..32 {
        vec.push(i);
    }
    assert!(vec.len() == 32);
    for i in 0..32 {
        assert!(vec[i as usize] == i);
    }
}

#[test]
fn large_push() {
    let mut vec = BTreeVec::<u8, 8>::new();
    for i in 0..128 {
        vec.push(i);
    }
    assert!(vec.len() == 128);
    for i in 0..128 {
        assert!(vec[i as usize] == i);
    }
}

#[test]
fn basic_insert_front() {
    let mut vec = BTreeVec::<u8, 7>::new();
    for i in 0..8 {
        vec.insert(0, i);
    }
    assert!(vec.len() == 8);
    for i in 0..8 {
        assert!(vec[i as usize] == 8 - i - 1);
    }
}

#[test]
fn medium_insert_front() {
    let mut vec = BTreeVec::<u8, 7>::new();
    for i in 0..32 {
        vec.insert(0, i);
    }
    assert!(vec.len() == 32);
    for i in 0..32 {
        assert!(vec[i as usize] == 32 - i - 1);
    }
}

#[test]
fn large_insert_front() {
    let mut vec = BTreeVec::<u8, 7>::new();
    for i in 0..128 {
        vec.insert(0, i);
    }
    assert!(vec.len() == 128);
    for i in 0..128 {
        assert!(vec[i as usize] == 128 - i - 1);
    }
}

#[test]
fn basic_insert_middle() {
    let mut v = Vec::new();
    let mut vec = BTreeVec::<u8, 7>::new();
    for i in 0..4 {
        vec.push(i);
        v.push(i);
    }
    for i in 0..4 {
        vec.insert(2 + i as usize, i + 10);
        v.insert(2 + i as usize, i + 10);
    }
    assert!(vec.len() == 8);
    for i in 0..8 {
        assert!(vec[i] == v[i]);
    }
}

#[test]
fn medium_insert_middle() {
    let mut v = Vec::new();
    let mut vec = BTreeVec::<u8, 8>::new();
    for i in 0..16 {
        vec.push(i);
        v.push(i);
    }
    for i in 0..16 {
        vec.insert(2 + i as usize, i + 100);
        v.insert(2 + i as usize, i + 100);
    }
    assert!(vec.len() == 32);
    for i in 0..16 {
        assert!(vec[i] == v[i]);
    }
}

#[test]
fn large_insert_middle() {
    let mut v = Vec::new();
    let mut vec = BTreeVec::<u8, 7>::new();
    for i in 0..64 {
        vec.push(i);
        v.push(i);
    }
    for i in 0..64 {
        vec.insert(2 + i as usize, i + 100);
        v.insert(2 + i as usize, i + 100);
    }
    assert!(vec.len() == 128);
    for i in 0..64 {
        assert!(vec[i] == v[i]);
    }
}
