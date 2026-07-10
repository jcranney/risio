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
// // #[cfg(feature = "rayon")]
// use rayon::{self,iter::{IntoParallelRefMutIterator, ParallelIterator, IntoParallelIterator}};

use std::marker::PhantomData;

pub use bindings::IMAGE;
use datatype::*;
use error::Error;

use crate::imagestreamio::{Image, byte_structs::TimeSpec};

/// A type that implements Accessor will contain a method that interacts with a
/// reference to an ImageStreamIO IMAGE,
pub trait Accessor<'a> {
    type DTYPE: IsioDataType;
    fn name(&self) -> &str;
    fn image(&'a self) -> &'a Image<'a>;
    // fn image_mut(&'a mut self) -> &'a mut Image<'a>;
    unsafe fn array(&'a self) -> &'a [Self::DTYPE] {
        Self::DTYPE::from_bytes(unsafe { self.image().array.get().read() })
    }
    unsafe fn modify<F>(&'a self, f: F) -> Result<(), Error>
    where
        F: FnMut(&mut Self::DTYPE),
    {
        if unsafe { self.image().md.get().read().write } != 0 {
            return Err(Error::ImageIsBeingWritten(self.name().to_string()));
        } else {
            unsafe {
                self.image().md.get().read().write = 1;
            }
        }
        let array: &mut [Self::DTYPE] =
            Self::DTYPE::from_bytes_mut(unsafe { self.image().array.get().read() });
        array.iter_mut().for_each(f);
        unsafe { self.image().md.get().read().write = 0 };
        Ok(())
    }
    // // #[cfg(feature = "rayon")]
    // unsafe fn par_modify<F>(&'a self, f: F) -> Result<(), Error>
    // where
    //     F: FnMut(&mut Self::DTYPE),
    // {
    //     if unsafe { self.image().md.get().read().write } != 0 {
    //         return Err(Error::ImageIsBeingWritten(self.name().to_string()));
    //     } else {
    //         unsafe {
    //             self.image().md.get().read().write = 1;
    //         }
    //     }
    //     let array: &mut [Self::DTYPE] =
    //         Self::DTYPE::from_bytes_mut(unsafe { self.image().array.get().read() });
    //     array.par_iter_mut().for_each(f);
    //     unsafe { self.image().md.get().read().write = 0 };
    //     Ok(())
    // }
    unsafe fn sem_post(&'a self, idx: usize) {
        let mut s = unsafe { self.image().sem_file.get().read() }[idx];
        unsafe {
            libc::sem_post(&mut s);
        }
    }
    unsafe fn sem_wait(&'a self, idx: usize) {
        let mut s = unsafe { self.image().sem_file.get().read() }[idx];
        unsafe {
            libc::sem_wait(&mut s);
        }
    }
    unsafe fn array_mut(&'a mut self) -> &'a mut [Self::DTYPE] {
        Self::DTYPE::from_bytes_mut(unsafe { self.image().array.get().read() })
    }
}

pub enum SemIdx {
    One(usize),
    Some(Vec<usize>),
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
        let image = Image::create_new_image_from_scratch(
            name,
            shape,
            T::to_datatype(),
            10,
            ImageType::image(),
            0,
        )?;
        let found_dt = unsafe { image.md.get().read().datatype };
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
    pub fn open(name: &'a str) -> Result<Self, Error> {
        let image = Image::open_image(name)?;
        Ok(Self {
            _im_name: name.to_string(),
            _image: image,
            // _mmap: mmap,
            _phantom_data: PhantomData::<T>,
        })
    }
}

impl<'a, T: IsioDataType> Accessor<'a> for RawImage<'a, T> {
    type DTYPE = T;

    fn name(&self) -> &str {
        &self._im_name
    }

    fn image(&'a self) -> &'a Image<'a> {
        &self._image
    }

    // fn image_mut(&'a mut self) -> &'a mut Image<'a> {
    //     &mut self._image
    // }
}
