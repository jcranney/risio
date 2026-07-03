#[allow(
    non_snake_case,
    unnecessary_transmutes,
    non_upper_case_globals,
    non_camel_case_types
)]
mod bindings;
pub mod error;
pub mod imagestreamio;
pub mod datatype;

use std::{
    marker::PhantomData,
    slice::{from_raw_parts, from_raw_parts_mut},
};

use datatype::*;
use anyhow::Result;
pub use bindings::IMAGE;
use error::Error;
use memmap2::MmapMut;

pub trait Accessor {
    type T: IsioDataType;
    fn name(&self) -> &str;
    fn image(&self) -> &IMAGE;
    fn image_mut(&mut self) -> &mut IMAGE;
    fn array(&self) -> &[Self::T] {
        let bytes: &[Self::T] = Self::T::from_bytes(unsafe {
            from_raw_parts(
                self.image().array.UI8,
                self.image().md.read().imdatamemsize as usize,
            )
        });
        bytes
    }
    fn array_mut(&mut self) -> &mut [Self::T] {
        let bytes: &mut [Self::T] = Self::T::from_bytes_mut(unsafe {
            from_raw_parts_mut(
                self.image().array.UI8,
                self.image().md.read().imdatamemsize as usize,
            )
        });
        bytes
    }
}

pub struct RawImage<T: IsioDataType> {
    pub _im_name: String,
    pub _image: IMAGE,
    _phantom_data: PhantomData<T>,
    _mmap: MmapMut,
}

impl<T: IsioDataType> RawImage<T> {
    /// Create a new image with the specified name and shape. Returns an error
    /// if the image already exists.
    pub fn create_new(name: &str, shape: &[usize]) -> Result<Self> {
        let (image, mmap) = IMAGE::create_new_image_from_scratch(
            name,
            shape,
            T::to_datatype(),
            10,
            ImageType::image(),
            0,
        )?;
        let found_dt = unsafe { image.md.read().datatype };
        if Into::<u8>::into(T::to_datatype()) != found_dt {
            return Err(Error::MismatchDataType {
                expected: T::to_datatype(),
                found: DataType::try_from(found_dt)?,
            }
            .into());
        }
        Ok(Self {
            _im_name: name.to_string(),
            _image: image,
            _mmap: mmap,
            _phantom_data: PhantomData::<T>,
        })
    }

    /// Open an image with a specified name. Returns an error if the image
    /// doesnt exist, or if it exists with the wrong datatype.
    pub fn open(name: &str) -> Result<Self> {
        let (image, mmap) = IMAGE::open_image(name)?;
        Ok(Self {
            _im_name: name.to_string(),
            _image: image,
            _mmap: mmap,
            _phantom_data: PhantomData::<T>,
        })
    }
}

impl<T: IsioDataType> Accessor for RawImage<T> {
    type T = T;

    fn name(&self) -> &str {
        &self._im_name
    }

    fn image(&self) -> &IMAGE {
        &self._image
    }

    fn image_mut(&mut self) -> &mut IMAGE {
        &mut self._image
    }
}
