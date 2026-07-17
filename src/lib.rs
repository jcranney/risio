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

/// A type that implements Accessor attempts a form of "interior mutability",
/// where interaction with shared memory is restricted to a handful of
/// well-considered methods. By restricting the shared-memory
/// interaction to methods provided by Accessor, the chances of accidental
/// race conditions and improper implementation are greatly reduced, but since
/// the user requires fast access to shared memory, it's unlikely that any
/// implementation can ever be truly "safe".
pub trait Accessor<'a> {
    type DTYPE: IsioDataType;

    /// returns an Image object, which contains many `UnsafeCell`s that contain
    /// mutable references pointing to shared memory.
    unsafe fn image(&self) -> &Image<'a>;
    unsafe fn image_mut(&mut self) -> &mut Image<'a>;

    /// Returns the memory mapped image data as an immutable slice. The elements
    /// of this slice are directly mapped to the bytes in the shm image. This
    /// is still unsafe, since it is readily subject to race conditions from
    /// other processes.
    unsafe fn array(&'a self) -> &'a [Self::DTYPE] {
        Self::DTYPE::from_bytes(unsafe { self.image().array })
    }

    /// formatted name from Image MetaData
    unsafe fn name(&self) -> String {
        self.metadata()
            .name
            .iter()
            .map_while(|x| match x {
                0 => None,
                x => Some(*x as u8 as char),
            })
            .collect()
    }

    /// Modify shared memory data in-place by passing a closure that mutates
    /// the existing element, possibly using the index of that element in it's
    /// computation which is the first argument of the closure.
    unsafe fn modify<F>(&mut self, f: F) -> Result<(), Error>
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
    unsafe fn par_modify<F>(&mut self, f: F) -> Result<(), Error>
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

    /// Typically used by the producer of an image to flag with the image
    /// consumers that the data in the image is ready to be processed.
    /// `sem_post` increments the value of the underlying semaphore by 1,
    /// potentially freeing a caller of `sem_wait` if the semaphore value was
    /// 0 beforehand.
    ///
    /// This function posts to all semaphores in this shm image, which is a
    /// common pattern for "single producer, multiple consumer" shared memory.
    unsafe fn sem_post_all(&mut self) {
        for i in 0..self.metadata().sem {
            unsafe { self.sem_post_one(i as usize) };
        }
    }
    /// Post to a single semaphores in this shm image, which can be useful in
    /// "single producer, single consumer" configurations, or when many
    /// processes modify the image data in-place, and the semaphores are used
    /// to establish a "sequence" of work.
    unsafe fn sem_post_one(&mut self, idx: usize) {
        let s = &mut unsafe { self.image_mut().sem_file[idx] };
        unsafe { Self::_sem_post(s) };
    }
    unsafe fn _sem_post(s: &mut libc::sem_t) {
        let result = unsafe { libc::sem_post(s) };
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
        let s = &mut unsafe { self.image_mut().sem_file[idx] };
        let result = unsafe { libc::sem_wait(s) };
        if result < 0 {
            panic!();
        }
    }

    /// Read the value of a given semaphore (not typically used).
    unsafe fn sem_val(&mut self, idx: usize) -> i32 {
        let mut sval: i32 = 0;
        let s = &mut unsafe { self.image_mut().sem_file[idx] };
        let result = unsafe { libc::sem_getvalue(s, &mut sval) };
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
        while unsafe { self.sem_val(idx) } > 0 {
            unsafe { self.sem_wait(idx) };
        }
    }

    /// Returns the memory mapped image data as a mutable slice. The elements of
    /// this slice are directly mapped to the bytes in the shm image. This is
    /// the least-safe way to access image data, as it requires you to manage
    /// the "md.write" flag.
    fn array_mut(&'a mut self) -> &'a mut [Self::DTYPE] {
        Self::DTYPE::from_bytes_mut(unsafe { self.image_mut().array })
    }

    /// Returns a clone of the ImageMetadata.
    fn metadata(&self) -> ImageMetadata {
        unsafe { self.image().md.clone() }
    }
}

pub enum SemIdx {
    Only(usize),
    All,
}

pub struct RawImage<'a, T: IsioDataType> {
    pub _im_name: String,
    pub _image: Image<'a>,
    _phantom_data: PhantomData<T>,
    // _mmap: MmapMut,
}

impl<'a, T: IsioDataType> RawImage<'a, T> {
    /// Create a new image with the specified name and shape. Returns an error
    /// if the image already exists.
    pub fn create_new(name: &str, shape: &[usize]) -> Result<Self, Error> {
        let image = Image::new_shared(
            name,
            name,
            shape,
            T::to_datatype(),
            10,
            ImageType::image(),
            0,
        )?;
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

    /// Open an image with a specified name. Returns an error if the image
    /// doesnt exist, or if it exists with the wrong datatype.
    pub fn open(name: &str) -> Result<Self, Error> {
        let image = Image::open_image(name)?;
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

impl<'a, T: IsioDataType> Accessor<'a> for RawImage<'a, T> {
    type DTYPE = T;

    unsafe fn image(&self) -> &Image<'a> {
        &self._image
    }

    unsafe fn image_mut(&mut self) -> &mut Image<'a> {
        &mut self._image
    }
}
