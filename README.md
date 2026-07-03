# `risio`

The goal of this project is to wrap the main components of ImageStreamIO somehow in rust. My goal is to get to a point where I can write a simple rust program that:
 - opens two ImageSteamIO shared memory object,
 - waits for the first object to be updated,
 - performs a computation on that object,
 - writes the result back to the second object.

I'm sure this will be non-trivial, due to all of the rust safety guarantees seeming to be at odds with direct `shm` interaction, but let's see.

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
├── build.rs  # rules for extracting extern C library
├── flake.lock
├── flake.nix  # providing a dev shell and strict build testing
├── libImageStreamIO  # git submodule containing C library
│   └── ...
├── src
│   ├── bindings.rs  # rust bindings for libImageStreamIO
│   ├── imagestreamio.rs  # (goal) safe interface for IMAGE data in SHM
│   └── lib.rs  # bringing it all together
├── examples
│   └── simple_io.rs
├── benches
│   └── bench.rs
├── tests
│   └── simple_test.rs
└── UNLICENSE
```