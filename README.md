# `risio`

The goal of this project is to wrap the main components of ImageStreamIO somehow in rust. My goal is to get to a point where I can write a simple rust program that:
 - opens two ImageSteamIO shared memory object,
 - waits for the first object to be updated,
 - performs a computation on that object,
 - writes the result back to the second object.

I'm sure this will be non-trivial, due to all of the rust safety guarantees seeming to be at odds with direct `shm` interaction, but let's see.

## Inpsiration
For the maturin/rust/python project layout and for general correctness, I'm following some patterns from [github.com:ijl/osrson](https://github.com/ijl/orjson/tree/master). I reserve the right to flip that, but currently the project layout aims to be:
```bash
├── README.md
├── UNLICENSE
├── pyproject.toml
├── Cargo.toml
├── flake.nix  # devShell and packaged project
│
├── build.rs
├── libImageStreamIO  # git submodule containing C library
│   └── ...
│
├── src  # rust source files
│   ├── bindings.rs  # rust bindings for libImageStreamIO
│   ├── lib.rs  # rust wrapper for C bindings
│   └── python.rs  # rust interface for PyO3/Python
│
├── pysrc  # python source files
│   └── risio
├── test  # python tests
│   ├── test_correctness.py
│   └── ...
└── ...  # all the automagic fluff
```