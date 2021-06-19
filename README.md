# thread-safe

This crate implements a simple container that allows thread-unsafe variables to be sent across threads, as long
as they are only directly accessed from the origin thread. This crate allows for `breadthread` to not have any
unsafe code in it.

## License

This crate is dual-licensed under the MIT and Apache 2.0 Licenses, just like Rust proper is.
