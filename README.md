# `PinList`(s)

This crate aims to provide a safe and easy version of intrusive linked lists,
when you don't need to do anything particularly tricky with them, particularly
in an embedded context where those nodes might live on the stack (or
inside a `Task`).

It builds on top of the [`cordyceps`] crate, an excellent crate for doing
tricky things with intrusive data structures, including intrusive linked
lists.

[`cordyceps`]: https://docs.rs/cordyceps

## Versions

There is currently one version of "PinList" provided, [`blocking`], which
uses a scoped blocking mutex to mediate access. This allows for access to
the contents of the list using iterators while the mutex is locked.

This is intended for cases where you only need access for brief times, for
example to store/wake a Waker, or iterate over attached items and try to
push/pop data in a failable way.

In the future, there will also be a version that mediates access with an
async mutex instead of a blocking one, with the tradeoff that nodes must
be static, to avoid nodes ever being dropped.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
