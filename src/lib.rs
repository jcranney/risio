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

use std::{
    marker::PhantomData,
    slice::{from_raw_parts, from_raw_parts_mut},
};

use anyhow::Result;
pub use bindings::IMAGE;
use datatype::*;
use error::Error;
use memmap2::MmapMut;

use crate::imagestreamio::{
    Image,
    byte_structs::{CBFrameMetadata, ImageKeyword, ImageMetadata, StreamProcTrace, TimeSpec},
};

/// Contrary to the C library, here we will define a trait that delivers a
/// similar interface as the ImageStreamIO `IMAGE`, but with some rusty
/// modifications to (WIP) gaurantee memory safety.
///
/// The rationale here for defining a trait is that I plan to try multiple
/// types of accessors for the shared memory, and potentially to allow users to
/// write their own while still maintaining compatibility with orthogonal RTC
/// software.
pub trait ShmImage<'a> {
    /// The datatype of the SHM Image
    type T: IsioDataType;

    /// Read-only access to the underlying shared memory, in the representation
    /// corresponding to the defined datatype.
    fn array(&self) -> &[Self::T];
    /// Read/write access to the underlying shared memory, in the representation
    /// correspodning to the defined datatype.
    fn array_mut(&mut self) -> &mut [Self::T];

    /// Read-only access to the Image Metadata in the underlying shared memory
    fn md(&self) -> &ImageMetadata;
    /// Read/write access to the Image Metadata in the underlying shared memory
    fn md_mut(&mut self) -> &mut ImageMetadata;

    /// local name of the image, doesn't need to match the shm file name.
    fn name(&self) -> &str;

    /// if shared memory, file descriptor
    fn shmfd(&mut self) -> Option<&mut std::fs::File>;

    /// semaphore for logging
    fn semlog(&mut self) -> &mut bindings::sem_t;

    /// semaphores for computation
    fn sem(&mut self) -> &mut [bindings::sem_t];

    /// image keywords
    fn kw(&mut self) -> &mut [ImageKeyword];

    /// PID of process that read shared memory stream
    /// Initialized at 0. Otherwise, when process is waiting on semaphore,
    /// its PID is written in this array. The array can be used to look for
    /// available semaphores.
    fn sem_read_pid(&mut self) -> &mut [bindings::pid_t];

    /// PID of processes that are posting the semaphores (JC: I guess there should only be one?)
    fn sem_write_pid(&mut self) -> &mut [bindings::pid_t];

    /// semaphore control, written by writer to control semaphore behavior.
    /// See SEMAPHORE_CONTROL_XXX defines for details
    fn sem_ctrl(&mut self) -> &mut u32;

    /// semaphore status, written by readers to report back to stream what is their current status.
    /// See SEMAPHORE_STATUS_XXX defines for details
    fn sem_status(&mut self) -> &mut u32;

    // array to keep track of stream history/depedencies
    fn stream_proc_trace(&mut self) -> &mut [StreamProcTrace];
    /// flag for each slice if needed (depends on imagetype)
    fn flag_array(&mut self) -> &mut u64;
    /// For circular buffer: counter array for circular buffer, copy of cnt0 onto slice index
    fn cnt_array(&mut self) -> &mut u64;
    /// For each slice index: time at which data was acquires/created.
    /// This time CAN be copied from input to output
    fn a_time_array(&mut self) -> &mut [TimeSpec];
    /// For each slice index: time at which data was written.
    /// This time CAN be copied from input to output
    fn write_time_array(&mut self) -> &mut [TimeSpec];

    // Circular Buffer (CB) option
    // if CBsize>0, recent frames are memcpied in circular buffer
    // recent frames may be accessed in small CB for logging.
    /// array of CB metadata
    fn circ_buff_md(&mut self) -> &mut [CBFrameMetadata];
    /// data storage for circ buffer
    fn cb_im_data(&self) -> &[Self::T];
    /// mutabel data storage for circ buffer
    fn cb_im_data_mut(&mut self) -> &mut [Self::T];
}

/// A type that implements Accessor will contain a method that retrieves a
/// reference to an ImageStreamIO IMAGE,
pub trait Accessor<'a> {
    type T: IsioDataType;
    fn name(&self) -> &str;
    fn image(&self) -> &Image<'a>;
    fn image_mut(&mut self) -> &mut Image<'a>;
    fn array(&'a self) -> &'a [Self::T] {
        let bytes: &[Self::T] = Self::T::from_bytes(self.image().array);
        bytes
    }
    fn array_mut(&'a mut self) -> &'a mut [Self::T] {
        let bytes: &mut [Self::T] = Self::T::from_bytes_mut(self.image_mut().array);
        bytes
    }
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
    pub fn create_new(name: &str, shape: &[usize]) -> Result<Self> {
        let image = Image::create_new_image_from_scratch(
            name,
            shape,
            T::to_datatype(),
            10,
            ImageType::image(),
            0,
        )?;
        let found_dt = unsafe { image.md.datatype };
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
            // _mmap: mmap,
            _phantom_data: PhantomData::<T>,
        })
    }

    /// Open an image with a specified name. Returns an error if the image
    /// doesnt exist, or if it exists with the wrong datatype.
    pub fn open(name: &str) -> Result<Self> {
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
    type T = T;
    
    fn name(&self) -> &str {
        &self._im_name
    }
    
    fn image(&self) -> &Image<'a> {
        &self._image
    }
    
    fn image_mut(&mut self) -> &mut Image<'a> {
        &mut self._image
    }
}
