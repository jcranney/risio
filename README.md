# `risio`

The goal of this project is to wrap the main components of ImageStreamIO somehow in rust. My goal is to get to a point where I can write a simple rust program that:
 - opens two ImageSteamIO shared memory object,
 - waits for the first object to be updated,
 - performs a computation on that object,
 - writes the result back to the second object.

I'm sure this will be non-trivial, due to all of the rust safety guarantees seeming to be at odds with direct `shm` interaction, but let's see.

## Status
 - There are now no dependencies on external C ImageStreamIO, simplifying the build and install significantly,
 - Currently fleshing out an "as safe as possible" unsafe interface on the underlying memory mapped data. See:
   - `./src/lib.rs`
 - Basic shared memory access and semaphore posting/waiting is working, but thoroughly under-tested. Try executing:
   - `cargo run --example poster`
   - `cargo run --example waiter`

   from two separate shells.



## Package Layout
This crate will serve mostly as a library, exposing only necessary interfaces
and trying to keep those interfaces as stable as possible for use in other
crates.

The project follows, as close as possible, the canonical "library crate" format:

```bash
.
├── README.md
├── Cargo.lock
├── Cargo.toml
├── flake.lock
├── flake.nix  # providing a dev shell and strict build testing
├── src
│   ├── bindings.rs  # rust bindings for libImageStreamIO
│   ├── imagestreamio.rs  # (goal) safe interface for IMAGE data in SHM
│   ├── lib.rs  # bringing it all together
│   └── ...
├── examples
│   ├── simple_io.rs
│   └── ...
├── benches
│   └── bench.rs
├── tests
│   └── simple_test.rs
└── UNLICENSE
```