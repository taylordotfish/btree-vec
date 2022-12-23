btree-vec
=========

This crate provides a growable array (vector) implemented using a B-tree
(more specifically, a B+ tree). It provides non-amortized O(log n) random
accesses, insertions, and removals, as well as O(n) iteration. The
branching factor is also customizable.

The design is similar to [unsorted counted B-trees][cb] as described by
Simon Tatham.

[cb]: https://www.chiark.greenend.org.uk/~sgtatham/algorithms/cbtree.html

For now, the vector supports insertions and removals only of single
elements, but bulk operations, including implementations of [`Extend`]
and [`FromIterator`], may be added in the future.

Example
-------

```rust
let mut vec = BTreeVec::new();
for i in 0..20 {
    vec.push(i);
}
for i in 0..10 {
    assert!(vec.remove(i) == i * 2);
}
for i in 0..10 {
    assert!(vec[i] == i * 2 + 1);
}
for i in 0..10 {
    vec.insert(i * 2, i * 2);
}
assert!(vec.len() == 20);
for (i, n) in vec.iter().copied().enumerate() {
    assert!(i == n);
}
```

Crate features
--------------

If the crate feature `dropck_eyepatch` is enabled, items in a [`BTreeVec`]
can contain references with the same life as the vector itself. This
requires Rust nightly, as the unstable language feature [`dropck_eyepatch`]
must be used.

If the crate feature `allocator_api` is enabled, you can configure
[`BTreeVec`] with the unstable [`Allocator`] trait. Alternatively, if the
feature `allocator-fallback` is enabled, this crate will use the allocator
API provided by [allocator-fallback] instead of the standard library’s.

[`dropck_eyepatch`]: https://github.com/rust-lang/rust/issues/34761
[allocator-fallback]: https://docs.rs/allocator-fallback

[`BTreeVec`]: https://docs.rs/btree-vec/0.2/btree_vec/struct.BTreeVec.html
[`Extend`]: https://doc.rust-lang.org/std/iter/trait.Extend.html
[`FromIterator`]: https://doc.rust-lang.org/std/iter/trait.FromIterator.html
[`Allocator`]: https://doc.rust-lang.org/stable/std/alloc/trait.Allocator.html
