//! # risio
//! This crate is intended to be a rust implementation of the C [ImageStreamIO](https://github.com/milk-org/ImageStreamIO)
//! library (ISIO), targetting real-time control software for adaptive optics systems
//! along with other high-performance astronomical instrumentation pipelines. This is currently
//! a hobby project maintained on an on-demand basis. If you intend to rely on this
//! crate, please reach out to me via github issues or any other channel you have
//! available.
//!
//! Examples of how to use this crate are included in the git repository, so please
//! check them out. The main use case for the ISIO library is to allow a user
//! to develop multi-process/parallel pipelines with strict synchronicity requirements,
//! when large amounts of data is shared between the processes.
//!
//! In the rust eco-system, folks often invoke the `golang` proverb:
//!  > *Do not communicate by sharing memory; instead, share memory by communicating.*
//!
//! I don't personally have a strong opinion on sharing memory vs messaging, but
//! I do know that there is little to no chance for the adaptive optics community
//! to expunge the shared memory ideology overnight. Rather than to fight a losing
//! battle that I am not particularly passionate about, instead I'd rather focus
//! my efforts on providing a "safer" shared memory interface for users of
//! the prolific ImageStreamIO library. Since shared memory is inherently unsafe
//! and rust cannot guarantee that other processes will follow safe procedures,
//! this crate is littered with unsafe functions. It is my opinion, and indeed
//! the goal of this project, that the rust interface to shared memory images
//! via `risio` is at-least-as-safe as the C implementation, and explicit where
//! it is unsafe. It is possible that I have over-specified some functions as unsafe when they
//! are not, though I have tried to avoid that.
//!
//! A simple example for creating a shared memory image in the ISIO format is given
//! below, followed by another example that could be ran in a separate process,
//! connecting to the same underlying shared memory. For more examples, see the
//! git repository [examples](https://github.com/jcranney/risio/tree/main/examples).
//!
//! Creating a new shared memory image (and failing safely otherwise), updating
//! the image and then notifying other processes via semaphore post:
//! ```
//! use risio::prelude::*;
//! use rand::random;
//! // create a new image at `/dev/shm/noise.im.shm`
//! let mut image = match ShmImage::<f64>::create_new("noise", &[100, 100]) {
//!     Ok(im) => im,
//!     Err(_) => ShmImage::<f64>::open("noise").unwrap(),
//! };
//! // mutate the data in a loop:
//! for _ in 0..10 {
//!     // set a random value for each pixel
//!     unsafe { image.modify(|(_idx, value)| *value = random()) }.unwrap();
//!     // post on all semaphores so that waiting processes are released
//!     unsafe { image.sem_post_all() };
//!     // sleep for a moment
//!     std::thread::sleep(std::time::Duration::from_millis(1));
//! }
//! ```
//!
//! Connecting to an existing shared memory image and waiting on the semaphore:
//! ```
//! use risio::prelude::*;
//! let mut image: ShmImage<f64> = ShmImage::open("noise").unwrap();
//! unsafe { image.sem_flush(0) };
//! // uncomment this line if you want to wait on semaphore before proceeding:
//! // unsafe { image.sem_wait(0) };
//! let x: f64 =
//!     unsafe { image.array().iter().sum::<f64>() } / unsafe { image.array().len() } as f64;
//! println!("mean(im) = {:0.8}", x);
//! ```

// NOTE: the above sem_wait call will freeze the docs compilation if not run in
// parallel, hence why the sem_wait should be commented out. I'm not sure if the
// github workers are single threaded or not.

#[allow(
    non_snake_case,
    unnecessary_transmutes,
    non_upper_case_globals,
    non_camel_case_types
)]
mod bindings;
pub mod datatype;
pub mod error;
pub mod imagestreamio;
use memmap2::MmapMut;
#[cfg(feature = "rayon")]
use rayon::{
    self,
    iter::{IndexedParallelIterator, IntoParallelRefMutIterator, ParallelIterator},
};

use datatype::*;
use error::Error;
use std::marker::PhantomData;

use crate::imagestreamio::{
    Image,
    byte_structs::{ImageMetadata, TimeSpec},
};

pub mod prelude {
    pub use crate::{RawAccessor, Accessor, LocalImage, ShmImage};
}

/// A raw accessor is not type-aware, and is something like a "mid-level"
/// abstraction. In most cases, you will want to use the `Accessor` trait
/// instead, but this can be useful if you are developping a bytes-oriented
/// interface around a shared-memory image.
pub trait RawAccessor<'a, T>
where
    T: 'a,
{
    /// returns an Image object, which contains many `UnsafeCell`s that contain
    /// mutable references pointing to shared memory.
    unsafe fn image(&self) -> &Image<'a, T>;
    unsafe fn image_mut(&mut self) -> &mut Image<'a, T>;

    /// Returns the memory mapped image data as an immutable slice. The elements
    /// of this slice are directly mapped to the bytes in the shm image. This
    /// is still unsafe, since it is readily subject to race conditions from
    /// other processes.
    unsafe fn bytes(&'a self) -> &'a [u8] {
        <u8>::from_bytes(unsafe { self.image().array })
    }

    /// Returns the memory mapped image data as a mutable slice. The elements of
    /// this slice are directly mapped to the bytes in the shm image. This is
    /// the least-safe way to access image data, as it requires you to manage
    /// the "md.write" flag.
    unsafe fn bytes_mut(&'a mut self) -> &'a mut [u8] {
        <u8>::from_bytes_mut(unsafe { self.image_mut().array })
    }

    /// formatted name from Image MetaData
    unsafe fn name(&'a self) -> String {
        self.metadata()
            .name
            .clone()
            .iter()
            .map_while(|x| match x {
                0 => None,
                x => Some(*x as u8 as char),
            })
            .collect::<String>()
    }

    /// Typically used by the producer of an image to flag with the image
    /// consumers that the data in the image is ready to be processed.
    /// `sem_post` increments the value of the underlying semaphore by 1,
    /// potentially freeing a caller of `sem_wait` if the semaphore value was
    /// 0 beforehand.
    ///
    /// This function posts to all semaphores in this shm image, which is a
    /// common pattern for "single producer, multiple consumer" shared memory.
    unsafe fn sem_post_all(&mut self) {
        for sem in unsafe { self.image_mut().sem_file.iter_mut() } {
            let result = unsafe { libc::sem_post(sem) };
            if result < 0 {
                panic!();
            }
        }
    }
    /// Post to a single semaphores in this shm image, which can be useful in
    /// "single producer, single consumer" configurations, or when many
    /// processes modify the image data in-place, and the semaphores are used
    /// to establish a "sequence" of work.
    unsafe fn sem_post_one(&mut self, idx: usize) {
        let sem = unsafe { &mut self.image_mut().sem_file[idx] };
        let result = unsafe { libc::sem_post(sem) };
        if result < 0 {
            panic!();
        }
    }

    /// Typically used by the consumer of an image to enable image pipeline
    /// synchronisation. If the underlying semaphore has a value of greater
    /// than 0, then `sem_wait` will reduce the semaphore value by 1, and
    /// immediately return. If the semaphore has a value equal to 1, then the
    /// call to `sem_wait` will block until another thread/process has called
    /// `sem_post` (which increases the underlying semaphore value by 1). If
    /// mutiple consumers have called `sem_wait` on the exact same semaphore,
    /// then only one of these callers will be freed when the producer
    /// `sem_post`s.
    unsafe fn sem_wait(&mut self, idx: usize) {
        let sem = unsafe { &mut self.image_mut().sem_file[idx] };
        let result = unsafe { libc::sem_wait(sem) };
        if result < 0 {
            panic!();
        }
    }

    /// Read the value of a given semaphore (not typically used).
    unsafe fn sem_val(&mut self, idx: usize) -> i32 {
        let mut sval: i32 = 0;
        let sem = unsafe { &mut self.image_mut().sem_file[idx] };
        let result = unsafe { libc::sem_getvalue(sem, &mut sval) };
        if result < 0 {
            panic!();
        }
        sval
    }

    /// Typically used by the consumer of the image when they are about to enter
    /// a loop, resets the semaphore value to 0, which guarantees that the next
    /// time the  consumer calls "sem_wait", it will block until the semaphore
    /// has been posted.
    unsafe fn sem_flush(&mut self, idx: usize) {
        let sem = unsafe { &mut self.image_mut().sem_file[idx] };
        loop {
            let mut sval: i32 = 0;
            let result = unsafe { libc::sem_getvalue(sem, &mut sval) };
            if result < 0 {
                panic!();
            }
            if sval == 0 {
                break;
            }
            unsafe { libc::sem_wait(sem) };
        }
    }

    /// Returns a reference to the image metadata
    fn metadata(&'a self) -> &'a ImageMetadata {
        unsafe { self.image() }.md
    }

    /// Returns a mutable reference to the image metadata
    fn metadata_mut(&'a mut self) -> &'a mut ImageMetadata {
        unsafe { self.image_mut().md }
    }
}

/// A type that implements Accessor attempts a form of "interior mutability",
/// where interaction with shared memory is restricted to a handful of
/// well-considered methods. By restricting the shared-memory
/// interaction to methods provided by Accessor, the chances of accidental
/// race conditions and improper implementation are greatly reduced, but since
/// the user requires fast access to shared memory, it's unlikely that any
/// implementation can ever be truly "safe".
pub trait Accessor<'a, T>: RawAccessor<'a, T>
where
    T: 'a,
{
    type DTYPE: IsioDataType;

    /// Returns the memory mapped image data as an immutable slice. The elements
    /// of this slice are directly mapped to the bytes in the shm image. This
    /// is still unsafe, since it is readily subject to race conditions from
    /// other processes.
    unsafe fn array(&'a self) -> &'a [Self::DTYPE] {
        Self::DTYPE::from_bytes(unsafe { self.image().array })
    }

    /// Returns the memory mapped image data as a mutable slice. The elements of
    /// this slice are directly mapped to the bytes in the shm image. This is
    /// the least-safe way to access image data, as it requires you to manage
    /// the "md.write" flag.
    unsafe fn array_mut(&'a mut self) -> &'a mut [Self::DTYPE] {
        Self::DTYPE::from_bytes_mut(unsafe { self.image_mut().array })
    }

    /// Modify shared memory data in-place by passing a closure that mutates
    /// the existing element, possibly using the index of that element in it's
    /// computation which is the first argument of the closure.
    unsafe fn modify<F>(&'a mut self, f: F) -> Result<(), Error>
    where
        F: Fn((usize, &mut Self::DTYPE)),
    {
        if { unsafe { self.image_mut().md.write } } != 0 {
            return Err(Error::ImageIsBeingWritten(unsafe {
                self.name().to_string()
            }));
        } else {
            unsafe { self.image_mut().md.write = 1 };
        }
        let array: &mut [Self::DTYPE] =
            Self::DTYPE::from_bytes_mut(unsafe { self.image_mut().array });
        array.iter_mut().enumerate().for_each(f);
        unsafe { self.image_mut().md.write = 0 };
        Ok(())
    }

    #[cfg(feature = "rayon")]
    /// Modify shared memory data in-place by passing a closure that mutates
    /// the existing element, possibly using the index of that element in it's
    /// computation which is the first argument of the closure.
    unsafe fn par_modify<F>(&'a mut self, f: F) -> Result<(), Error>
    where
        F: Fn((usize, &mut Self::DTYPE)) + Sync + Send,
    {
        if unsafe { self.image_mut().md.write } != 0 {
            return Err(Error::ImageIsBeingWritten(unsafe {
                self.name().to_string()
            }));
        } else {
            unsafe {
                self.image_mut().md.write = 1;
            }
        }
        let array: &mut [Self::DTYPE] =
            Self::DTYPE::from_bytes_mut(unsafe { self.image_mut().array });
        array.par_iter_mut().enumerate().for_each(f);
        unsafe { self.image_mut().md.write = 0 };
        Ok(())
    }
}

pub struct ShmImage<'a, T: IsioDataType> {
    pub _im_name: String,
    pub _image: Image<'a, memmap2::MmapMut>,
    _phantom_data: PhantomData<T>,
    // _mmap: MmapMut,
}

impl<'a, T: IsioDataType> ShmImage<'a, T> {
    /// Create a new image with the specified name and shape. Returns an error
    /// if the image already exists.
    pub fn create_new(name: &str, shape: &[usize]) -> Result<Self, Error> {
        let image = unsafe {
            Image::new_shm(
                name,
                name,
                shape,
                T::to_datatype(),
                10,
                ImageType::image(),
                0,
            )
        }?;
        Ok(Self {
            _im_name: name.to_string(),
            _image: image,
            // _mmap: mmap,
            _phantom_data: PhantomData::<T>,
        })
    }

    /// Open an image with a specified name. Returns an error if the image
    /// doesnt exist, or if it exists with the wrong datatype.
    pub fn open(name: &str) -> Result<Self, Error> {
        let image = unsafe { Image::open_image(name) }?;
        let found_dt = image.md.datatype;
        if Into::<u8>::into(T::to_datatype()) != found_dt {
            Err(Error::MismatchDataType {
                expected: T::to_datatype(),
                found: DataType::try_from(found_dt)?,
            })
        } else {
            Ok(Self {
                _im_name: name.to_string(),
                _image: image,
                // _mmap: mmap,
                _phantom_data: PhantomData::<T>,
            })
        }
    }
}

impl<'a, T: IsioDataType> RawAccessor<'a, MmapMut> for ShmImage<'a, T> {
    // type DTYPE = T;

    unsafe fn image(&self) -> &Image<'a, MmapMut> {
        &self._image
    }

    unsafe fn image_mut(&mut self) -> &mut Image<'a, MmapMut> {
        &mut self._image
    }
}

impl<'a, T: IsioDataType> Accessor<'a, MmapMut> for ShmImage<'a, T> {
    type DTYPE = T;
}

pub struct LocalImage<'a, T: IsioDataType> {
    pub _image: Image<'a, Vec<u8>>,
    _phantom_data: PhantomData<T>,
    // _mmap: MmapMut,
}

impl<'a, T: IsioDataType> LocalImage<'a, T> {
    /// Create a new image with the specified name and shape. Returns an error
    /// if the image already exists.
    pub fn create_new(name: &str, shape: &[usize]) -> Result<Self, Error> {
        let image = Image::new_local(name, shape, T::to_datatype(), 10, ImageType::image(), 0)?;
        Ok(Self {
            _image: image,
            _phantom_data: PhantomData::<T>,
        })
    }
}

impl<'a, T: IsioDataType> RawAccessor<'a, Vec<u8>> for LocalImage<'a, T> {
    // type DTYPE = T;

    unsafe fn image(&self) -> &Image<'a, Vec<u8>> {
        &self._image
    }

    unsafe fn image_mut(&mut self) -> &mut Image<'a, Vec<u8>> {
        &mut self._image
    }
}
impl<'a, T: IsioDataType> Accessor<'a, Vec<u8>> for LocalImage<'a, T> {
    type DTYPE = T;
}

#[cfg(test)]
mod tests {
    #[test]
    fn docs_example() -> Result<(), crate::Error> {
        use super::prelude::*;
        use rand::random;
        // create a new image at `/dev/shm/noise.im.shm`
        let mut image = match ShmImage::<f64>::create_new("noise", &[100, 100]) {
            Ok(im) => im,
            Err(_) => ShmImage::<f64>::open("noise")?,
        };
        // mutate the data in a loop:
        for _ in 0..10 {
            // set a random value for each pixel
            unsafe { image.modify(|(_idx, value)| *value = random()) }.unwrap();
            // post on all semaphores so that waiting processes are released
            unsafe { image.sem_post_all() };
            // sleep for a moment
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        Ok(())
    }
}
