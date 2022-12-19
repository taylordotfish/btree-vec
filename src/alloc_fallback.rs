/*
 * Copyright (C) 2022 taylor.fish <contact@taylor.fish>
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

use alloc::alloc::Layout;
use core::ptr::{self, NonNull};

mod sealed {
    pub trait Sealed {}
}

pub trait Allocator: sealed::Sealed {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, ()> {
        assert!(layout.size() != 0);
        NonNull::new(ptr::slice_from_raw_parts_mut(
            // SAFETY: We ensured that the size of the layout is not 0.
            unsafe { alloc::alloc::alloc(layout) },
            layout.size(),
        ))
        .ok_or(())
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: Ensured by caller.
        unsafe { alloc::alloc::dealloc(ptr.as_ptr(), layout) };
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Global;

impl sealed::Sealed for Global {}
impl Allocator for Global {}
