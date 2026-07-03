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

use std::slice::{from_raw_parts, from_raw_parts_mut};

use anyhow::Result;
pub use bindings::IMAGE;
use datatype::*;
use error::Error;
use memmap2::MmapMut;

pub enum ShmimSlice<'a> {
    U8(&'a [u8]),
    I8(&'a [i8]),
    U16(&'a [u16]),
    I16(&'a [i16]),
    U32(&'a [u32]),
    I32(&'a [i32]),
    U64(&'a [u64]),
    I64(&'a [i64]),
    F32(&'a [f32]),
    F64(&'a [f64]),
}

pub enum ShmimMutSlice<'a> {
    U8(&'a mut [u8]),
    I8(&'a mut [i8]),
    U16(&'a mut [u16]),
    I16(&'a mut [i16]),
    U32(&'a mut [u32]),
    I32(&'a mut [i32]),
    U64(&'a mut [u64]),
    I64(&'a mut [i64]),
    F32(&'a mut [f32]),
    F64(&'a mut [f64]),
}

pub trait Accessor {
    fn name(&self) -> &str;
    fn image(&self) -> &IMAGE;
    fn image_mut(&mut self) -> &mut IMAGE;
    fn array<'a>(&'a self) -> Result<ShmimSlice<'a>> {
        let x = unsafe {
            from_raw_parts(
                self.image().array.UI8,
                self.image().md.read().imdatamemsize as usize,
            )
        };
        Ok(
            match TryInto::<DataType>::try_into(unsafe { self.image().md.read().datatype }) {
                Ok(dt) => match dt {
                    DataType::U8 => ShmimSlice::U8(u8::from_bytes(x)),
                    DataType::I8 => ShmimSlice::I8(i8::from_bytes(x)),
                    DataType::U16 => ShmimSlice::U16(u16::from_bytes(x)),
                    DataType::I16 => ShmimSlice::I16(i16::from_bytes(x)),
                    DataType::U32 => ShmimSlice::U32(u32::from_bytes(x)),
                    DataType::I32 => ShmimSlice::I32(i32::from_bytes(x)),
                    DataType::U64 => ShmimSlice::U64(u64::from_bytes(x)),
                    DataType::I64 => ShmimSlice::I64(i64::from_bytes(x)),
                    DataType::F32 => ShmimSlice::F32(f32::from_bytes(x)),
                    DataType::F64 => ShmimSlice::F64(f64::from_bytes(x)),
                    DataType::C64 => unimplemented!(),
                    DataType::C128 => unimplemented!(),
                    DataType::F16 => unimplemented!(),
                },
                Err(e) => return Err(e.into()),
            },
        )
    }
    fn array_mut<'a>(&'a mut self) -> Result<ShmimMutSlice<'a>> {
        let x = unsafe {
            from_raw_parts_mut(
                self.image_mut().array.UI8,
                self.image().md.read().imdatamemsize as usize,
            )
        };
        Ok(
            match TryInto::<DataType>::try_into(unsafe { self.image().md.read().datatype }) {
                Ok(dt) => match dt {
                    DataType::U8 => ShmimMutSlice::U8(u8::from_bytes_mut(x)),
                    DataType::I8 => ShmimMutSlice::I8(i8::from_bytes_mut(x)),
                    DataType::U16 => ShmimMutSlice::U16(u16::from_bytes_mut(x)),
                    DataType::I16 => ShmimMutSlice::I16(i16::from_bytes_mut(x)),
                    DataType::U32 => ShmimMutSlice::U32(u32::from_bytes_mut(x)),
                    DataType::I32 => ShmimMutSlice::I32(i32::from_bytes_mut(x)),
                    DataType::U64 => ShmimMutSlice::U64(u64::from_bytes_mut(x)),
                    DataType::I64 => ShmimMutSlice::I64(i64::from_bytes_mut(x)),
                    DataType::F32 => ShmimMutSlice::F32(f32::from_bytes_mut(x)),
                    DataType::F64 => ShmimMutSlice::F64(f64::from_bytes_mut(x)),
                    DataType::C64 => unimplemented!(),
                    DataType::C128 => unimplemented!(),
                    DataType::F16 => unimplemented!(),
                },
                Err(e) => return Err(e.into()),
            },
        )
    }
}

pub struct RawImage {
    pub _im_name: String,
    pub _image: IMAGE,
    _mmap: MmapMut,
}

impl RawImage {
    /// Create a new image with the specified name and shape. Returns an error
    /// if the image already exists.
    pub fn create_new<T: IsioDataType>(name: &str, shape: &[usize]) -> Result<Self> {
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
        })
    }
}

impl Accessor for RawImage {
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
