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

## ImageStreamIO v2.00 Spec

Perhaps the ImageStreamIO specification is already documented somewhere, but I
can't find it. My attempt at reverse-engineering the spec from the implementation
is here, aided by the rust bindings, generated using `bindgen`. Note that the
bindings were generated for the ImageStreamIO version 2.00 (the latest stable
version as of July 2026).

### Shared Memory Data Layout

An image stored in shared memory will have the following layout and data sizes on
a 64-bit system:

| item              | size (B)                    | pad (B)                               | comments                                         |
| ----------------- | --------------------------- | ------------------------------------- | ------------------------------------------------ |
| metadata          | 384                         | 0                                     | Image Metadata, includes sizes                   |
| array             | `imdata_memsize`            | (8 - `imdata_memsize`) % 8            | Image data, padded to a multiple of 8 bytes      |
| keywords          | 128\*`nb_kw`                | 0                                     | Image keywords (array)                           |
| semfile           | 256\*`nb_sem`               | 0                                     | Actual semaphores in data, typicall `nb_sem==10` |
| semlog            | 256                         | 0                                     | Logging semaphore                                |
| sem_read_pid      | 4\*`nb_sem`                 | (8 - 4\*(`nb_sem`)) % 8               | PID of last semaphore reader (per semaphore)     |
| sem_write_pid     | 4\*`nb_sem`                 | (8 - 4\*(`nb_sem`)) % 8               | PID of last semaphore writer (per semaphore)     |
| sem_ctrl          | 4\*`nb_sem`                 | (8 - 4\*(`nb_sem`)) % 8               | TBD                                              |
| sem_status        | 4\*`nb_sem`                 | (8 - 4\*(`nb_sem`)) % 8               | TBD                                              |
| stream_proc_trace | 64\*`nb_proc_trace`         | 0                                     | TBD                                              |
| atimearray        | 16\*`size[2]`               | 0                                     | TBD                                              |
| writetimearray    | 16\*`size[2]`               | 0                                     | TBD                                              |
| cntarray          | 8\*`size[2]`                | 0                                     | TBD                                              |
| circ_buff_md      | 48\*`cb_size`               | 0                                     | Metadata for Circular Buffer                     |
| cb_imdata         | `imdata_memsize`\*`cb_size` | (8 - `imdata_memsize`\*`cb_size`) % 8 | Circular buffer data, padded to a multiple of 8  |

It's probably never useful to know this, but the total size in memory of an ImageStreamIO Image is (in bytes):

$$
\mathrm{total} = 640 + a(1+f) + \mathrm{pad}_8(a) + 128b + 272c + 4\mathrm{pad}_8(4c) - 64d + 40e +48f + \mathrm{pad}_8(af)
$$

where:

- $a$: `imdata_memsize`
- $b$: `nb_kw`
- $c$: `nb_sem`
- $d$: `nb_proc_trace`
- $e$: `size[2]`
- $f$: `cb_size`
- $\mathrm{pad}_8(x)$ is the function that computes the padding required to round $x$ up to a multiple of 8. E.g., $\mathrm{pad}_8(x) = (8-x) \% 8$

For example, a 6 by 8 by 10 image (`size=[6,8,10]`) with datatype `f64` (8 bytes per value), with 2 keywords, 10 semaphores (the default), 10 process traces (the default), and a circular buffer of size 20 (e.g., keeping the 20 most recent images in memory), will have:

- $a=3840=6\times8\times10\times8$: `imdata_memsize`
- $b=2$: `nb_kw`
- $c=10$: `nb_sem`
- $d=10$: `nb_proc_trace`
- $e=10$: `size[2]`
- $f=20$: `cb_size`

and a total size of $86256$ Bytes in memory.

It seems worth to note that if an image uses a 64-bit data type, and an even number of semaphores, the data will be tightly packed with no padding between sequential elements in memory. Many other conditions result in tightly packed data, so it is important to include some "odd" data dimensions and types to validate the implementation of this ISIO spec.

### Image Metadata Layout

Similarly, the image metadata structure has the following layout in memory:

| item            | size (B) | pad (B) | offset (B) | comments                                                                     |
| --------------- | -------- | ------- | ----------- | ---------------------------------------------------------------------------- |
| version         | 32       | 0       | 0           | Image Struct version from ISIO, utf-8                                        |
| name            | 80       | 0       | 32          | Image name, utf-8                                                            |
| naxis           | 1        | 3       | 112         | Number of axes (u8)                                                          |
| size            | 12       | 0       | 116         | Shape of image `[ u32 ; 3 ]`                                                 |
| nelement        | 8        | 0       | 128         | Number of elements in the image                                              |
| datatype        | 1        | 7       | 136         | enumeration of datatype (see `./src/datatypes.rs`)                           |
| imagetype       | 8        | 0       | 144         | enumeration of image type (see `./src/datatypes.rs`)                         |
| creationtime    | 16       | 0       | 152         | timestamp                                                                    |
| lastaccesstime  | 16       | 0       | 168         | timestamp                                                                    |
| atime           | 16       | 0       | 184         | timestamp                                                                    |
| writetime       | 16       | 0       | 200         | timestamp                                                                    |
| creator_pid     | 4        | 0       | 216         | pid of creator process                                                       |
| owner_pid       | 4        | 0       | 220         | pid of current owner                                                         |
| shared          | 1        | 7       | 224         | is the stream in shared memory?                                              |
| inode           | 8        | 0       | 232         | inode number if shared memory                                                |
| location        | 1        | 0       | 240         | -1 if in CPU memory, >=0 if in GPU memory on `location` device               |
| status          | 1        | 6       | 241         | 1 to log image (default); 0 : do not log: 2 : stop log (then goes back to 2) |
| flag            | 8        | 0       | 248         | bitmask, encodes read/write permissions.... NOTE: enum instead of defines    |
| logflag         | 1        | 1       | 256         | set to 1 to start logging                                                    |
| sem             | 2        | 0       | 258         | number of semaphores supported, specified at image creation                  |
| nb_proc_trace   | 2        | 2       | 260         | number of streamproctrace entries                                            |
| cnt0            | 8        | 0       | 264         | counter (incremented if image is updated)                                    |
| cnt1            | 8        | 0       | 272         | in 3D rolling buffer image, this is the last slice written                   |
| cnt2            | 8        | 0       | 280         | in cnt2-based syncronization, proceed until cnt0=cnt2                        |
| write           | 1        | 1       | 288         | 1 if image is being written                                                  |
| nb_kw           | 2        | 0       | 290         | number of keywords (max: 65536)                                              |
| cb_size         | 4        | 0       | 292         | circular buffer size (number of images to store in buffer)                   |
| cb_index        | 4        | 4       | 296         | current index of circular buffer                                             |
| cb_cycle        | 8        | 0       | 304         | ?                                                                            |
| imdatamemsize   | 8        | 0       | 312         | total size (in Bytes) of image data in memory                                |
| cuda_mem_handle | 64       | 0       | 320         | handle to cuda device memory?                                                |