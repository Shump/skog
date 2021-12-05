# skog

Swedish for "forest". A pure rust implementation of Adobe's stlab::forest data
structure.

## Introduction

A "forest" is a tree-like data structure. From [Adobe stlab's forest tutorial](https://stlab.adobe.com/group__asl__tutorials__forest.html):

> A forest is a node-based (bidirectional) data structure for the
> representation of a hierarchy. The parent/child relationship between the
> nodes is maintained in the container class, so any regular type can be stored
> within without affecting it. It is equipped with a host of powerful iterators
> for varied methods of traversing the hierarchy, each of which is described
> below.

The tutorial itself provides a good explanation of basic usage and fellow
author Foster Brereton's [introductory article](https://stlab.cc/2020/12/01/forest-introduction.html)
is a great insight into the structure of the forest data structure.

In particular, the forest data structure is a great tool for serializing
tree-like data. The `dirs` example provides a simple demonstration of this:


Clone Adobe's stlab repository:

```bash
git clone https://github.com/stlab/libraries.git
cd libraries
```

Run `dirs` against the repository:

```bash
cargo run --example dirs -- $PWD/stlab
```

Which prints the directory structure as xml:

```xml
<stlab>
	<cmake>
		<stlab>
			<coroutines>
			</coroutines>
			<development>
			</development>
		</stlab>
	</cmake>
	<stlab>
		<concurrency>
		</concurrency>
		<algorithm>
		</algorithm>
		<test>
		</test>
		<iterator>
		</iterator>
	</stlab>
	<test>
	</test>
</stlab>
```

## Cursors

Because of Rust's stricter ownership model, a direct translation of C++'s
iterators can not safely be exposed. Instead, this crate takes the same
approach [as proposed for Rust's LinkedList](https://github.com/rust-lang/rust/issues/58533)
and introduces "cursor" types.

Unlike Rust's iterators, it can freely move back-and-forth and sits between two
nodes in the forest. Furthermore, it also tracks the "edge" of the node and
therefore have similar semantics as C++ forest's iterators.

## Status

This is a very early release, and a lot of more tests are necessary.

Compared to the original C++ version, the API has also been very much trimmed
to the bare essentials, primarily because of the challenge introduced by Rust's
ownership model. But rather than exposing a highly experimental API with high
chance of changing in the future, I've chosen to release a smaller but more
stable API that I don't expect to change even after 1.0 release.

At current state, the only API change I expect in the future is to change
`size`'s `self` parameter from a mutable reference to an immutable reference.
It is currently a mutable reference so that this implementation can be the same
as the C++ implementation which mutates the inner `size` member if it's
detected to be out of date.
