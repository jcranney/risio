#[allow(
    non_snake_case,
    unnecessary_transmutes,
    non_upper_case_globals,
    non_camel_case_types
)]
pub mod bindings;
pub mod python;
pub mod sem;
pub mod shm;
pub mod ImageStreamIO;
use crate::bindings::{IMAGE, ZAXIS_MAPPING, ZAXIS_SPACIAL, ZAXIS_TEMPORAL, ZAXIS_UNDEF, ZAXIS_WAVELENGTH};
use enum_iterator::{Sequence, all};
use std::ffi::{CString, NulError};
use std::fmt::Debug;
use std::marker::PhantomData;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RisioError {
    #[error(
        "SHM object {name} already exists with different shape: {:?}, requested: {:?}",
        existing,
        requested
    )]
    ShmExistsWithOtherShape {
        name: String,
        existing: [u32; 3],
        requested: [u32; 3],
    },
    #[error(
        "SHM object {name} already exists with different type: {:?}, requested: {:?}",
        existing,
        requested
    )]
    ShmExistsWithOtherType {
        name: String,
        existing: DataType,
        requested: DataType,
    },
    #[error("Inavlid shape. Must have at most 3 axes. You requested {0}")]
    InvalidShape(usize),
    #[error("Core ImageStreamIO library error, unknown code: {0}")]
    ImageStreamIOError(i32),
    #[error("Name contains NULL bit pattern. {}", err.as_display())]
    InvalidName { err: NulError },
    #[error("Bad data type for existing object: {datatype}")]
    BadDataType { datatype: u8 },
    #[error("Core ImageStreamIO Error: Success")]
    Success,
    #[error("Core ImageStreamIO Error: Failure")]
    Failure,
    #[error("Core ImageStreamIO Error: InvalidArg")]
    InvalidArg,
    #[error("Core ImageStreamIO Error: NotImpl")]
    NotImpl,
    #[error("Core ImageStreamIO Error: BadAlloc")]
    BadAlloc,
    #[error("Core ImageStreamIO Error: FileOpen")]
    FileOpen,
    #[error("Core ImageStreamIO Error: FileSeek")]
    FileSeek,
    #[error("Core ImageStreamIO Error: FileWrite")]
    FileWrite,
    #[error("Core ImageStreamIO Error: FileExists")]
    FileExists,
    #[error("Core ImageStreamIO Error: Inode   ")]
    Inode,
    #[error("Core ImageStreamIO Error: Mmap")]
    Mmap,
    #[error("Core ImageStreamIO Error: SemInit")]
    SemInit,
    #[error("Core ImageStreamIO Error: Version")]
    Version,
}



// impl IMAGE {
//     fn new_empty() -> IMAGE;
// }
























impl From<NulError> for RisioError {
    fn from(value: NulError) -> Self {
        RisioError::InvalidName { err: value }
    }
}

impl RisioError {
    fn errno_to_error(value: i32) -> Result<(), Self> {
        match value {
            0 => Ok(()),
            x => Err(match x {
                1 => RisioError::Failure,
                10 => RisioError::InvalidArg,
                20 => RisioError::NotImpl,
                30 => RisioError::BadAlloc,
                40 => RisioError::FileOpen,
                42 => RisioError::FileSeek,
                44 => RisioError::FileWrite,
                46 => RisioError::FileExists,
                48 => RisioError::Inode,
                50 => RisioError::Mmap,
                60 => RisioError::SemInit,
                100 => RisioError::Version,
                _ => RisioError::ImageStreamIOError(x),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, Sequence, PartialEq)]
pub enum DataType {
    U8 = 1, // uint8_t
    I8,     // int8_t
    U16,    // uint16_t
    I16,    // int16_t
    U32,    // uint32_t
    I32,    // int32_t
    U64,    // uint64_t
    I64,    // int64_t,
    F32,    // IEEE 754 single-precision binary floating-point format: binary32
    F64,    // IEEE 754 double-precision binary floating-point format: binary64
    C64,    // complex_float
    C128,   // complex double
    F16,    // half precision floating-point
}

impl TryFrom<u8> for DataType {
    fn try_from(value: u8) -> Result<Self, RisioError> {
        for dt in all::<DataType>().collect::<Vec<DataType>>() {
            if dt as u8 == value {
                return Ok(dt);
            }
        }
        Err(RisioError::BadDataType { datatype: value })
    }

    type Error = RisioError;
}

impl DataType {
    fn typesize(&self) -> usize {
        match self {
            DataType::U8 => 1,
            DataType::I8 => 1,
            DataType::U16 => 2,
            DataType::I16 => 2,
            DataType::U32 => 4,
            DataType::I32 => 4,
            DataType::U64 => 8,
            DataType::I64 => 8,
            DataType::F32 => 4,
            DataType::F64 => 8,
            DataType::C64 => 8,
            DataType::C128 => 16,
            DataType::F16 => 2,
        }
    }
}
/// From ImageStreamIO: ImageStruct.h:
///
/// ```txt
/// 0x 0000 0000 0000 0001  Circular buffer, slice z axis is encoding time -> record writetime array
/// 0x 0000 0000 0000 0002  Image is mathematical vector or matrix
/// 0x 0000 0000 0000 0004  Image is stream received from another computer
/// 0x 0000 0000 0000 0008  Image is stream sent to other computer
///
/// 0x 0000 0000 000X 0000  axis[0] encoding code (0-15):
///    0: undefined (default)
///    1: spatial coordinate
///    2: temporal coordinate
///    3: wavelength coordinate
///    4: mapping index
/// ```
///

#[derive(Debug, Clone, Copy)]
pub struct ImageType {
    circular_buffer: bool,
    vector_or_matrix: bool,
    stream_from_other_computer: bool,
    stream_for_other_computer: bool,
    axis_encoding_code: ZAxisEncodingCode,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum ZAxisEncodingCode {
    #[default]
    Undefined,
    SpatialCoordinate,
    TemporalCoordinate,
    WavelengthCoordinate,
    MappingIndex,
}

impl From<ImageType> for u64 {
    fn from(value: ImageType) -> Self {
        let mut result: u64 = 0;
        result += if value.circular_buffer { 0x1 } else { 0 };
        result += if value.vector_or_matrix { 0x2 } else { 0 };
        result += if value.stream_from_other_computer {
            0x4
        } else {
            0
        };
        result += if value.stream_for_other_computer {
            0x8
        } else {
            0
        };
        result += match value.axis_encoding_code {
            ZAxisEncodingCode::Undefined => ZAXIS_UNDEF,
            ZAxisEncodingCode::SpatialCoordinate => ZAXIS_SPACIAL,
            ZAxisEncodingCode::TemporalCoordinate => ZAXIS_TEMPORAL,
            ZAxisEncodingCode::WavelengthCoordinate => ZAXIS_WAVELENGTH,
            ZAxisEncodingCode::MappingIndex => ZAXIS_MAPPING,
        } as u64;
        result
    }
}

impl Default for ImageType {
    fn default() -> Self {
        Self {
            circular_buffer: true,
            vector_or_matrix: true,
            stream_from_other_computer: false,
            stream_for_other_computer: false,
            axis_encoding_code: ZAxisEncodingCode::default(),
        }
    }
}

impl Debug for IMAGE {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IMAGE")
            .field(
                "name",
                &str::from_utf8(&self.name.map(|x| x as u8)).unwrap(),
            )
            .field("used", &self.used)
            .field("createcnt", &self.createcnt)
            .field("shmfd", &self.shmfd)
            .field("memsize", &self.memsize)
            .field("semlog", &self.semlog)
            .field("md", &self.md)
            .field("semptr", &self.semptr)
            .field("kw", &self.kw)
            .field("semfile", &self.semfile)
            .field("semReadPID", &self.semReadPID)
            .field("semWritePID", &self.semWritePID)
            .field("semctrl", &self.semctrl)
            .field("semstatus", &self.semstatus)
            .field("streamproctrace", &self.streamproctrace)
            .field("flagarray", &self.flagarray)
            .field("cntarray", &self.cntarray)
            .field("atimearray", &self.atimearray)
            .field("writetimearray", &self.writetimearray)
            .field("CircBuff_md", &self.CircBuff_md)
            .field("CBimdata", &self.CBimdata)
            .finish()
    }
}

#[derive(Debug)]
pub struct Image<T> {
    image: IMAGE,
    _data_type: PhantomData<T>,
    shape: [u32; 3],
}

impl<T: ValidImageType<T>> Image<T> {
    pub fn destroy_im(mut self) -> Result<(), RisioError> {
        let err = unsafe { bindings::ImageStreamIO_destroyIm(&mut self.image) };
        RisioError::errno_to_error(err)
    }

    pub fn destroy_by_name(name: &str) -> Result<(), RisioError> {
        let mut image = unsafe { Self::force_read_sharedmem_image(name) }?;
        let err = unsafe { bindings::ImageStreamIO_destroyIm(&mut image) };
        RisioError::errno_to_error(err)
    }

    // pub fn ImageStreamIO_openIm(image: *mut IMAGE, name: *const ::std::os::raw::c_char) -> errno_t;

    // pub fn ImageStreamIO_get_image_d_ptr(image: *mut IMAGE) -> *mut ::std::os::raw::c_void;

    pub fn open_or_create(name: &str, shape: &[u32]) -> Result<Self, RisioError> {
        let shape = Self::validate_shape(shape)?;

        match Self::read_sharedmem_image(name, &shape) {
            Ok(image) => Ok(image),
            Err(RisioError::FileOpen) => {
                // Doesn't exist, so we can create it:
                unsafe {
                    Self::create_image(
                        name,
                        shape.len() as i64,
                        shape,
                        -1,
                        true,
                        bindings::IMAGE_NB_SEMAPHORE as i32,
                        10,
                        ImageType::default(),
                        0,
                    )
                }
            }
            Err(e) => Err(e),
        }
    }

    fn validate_shape(shape: &[u32]) -> Result<[u32; 3], RisioError> {
        let shape = match shape.len() {
            0 => [1; 3],
            1 => [shape[0], 1, 1],
            2 => [shape[0], shape[1], 1],
            3 => [shape[0], shape[1], shape[2]],
            len => return Err(RisioError::InvalidShape(len)),
        };
        Ok(shape)
    }

    unsafe fn force_read_sharedmem_image(name: &str) -> Result<IMAGE, RisioError> {
        let name_c = CString::new(name)?;
        let mut image = std::mem::MaybeUninit::uninit();

        let err = unsafe {
            bindings::ImageStreamIO_read_sharedmem_image_toIMAGE(
                name_c.as_ptr(),
                image.as_mut_ptr(),
            )
        };
        RisioError::errno_to_error(err)?;
        let image = unsafe { image.assume_init() };
        Ok(image)
    }

    /// todo: extract shape from IMAGE object
    pub fn read_sharedmem_image(name: &str, shape: &[u32]) -> Result<Self, RisioError> {
        let shape = Self::validate_shape(shape)?;
        let name_c = CString::new(name)?;
        let mut image = std::mem::MaybeUninit::uninit();

        let err = unsafe {
            bindings::ImageStreamIO_read_sharedmem_image_toIMAGE(
                name_c.as_ptr(),
                image.as_mut_ptr(),
            )
        };
        RisioError::errno_to_error(err)?;
        let image = unsafe { image.assume_init() };

        let existing_shape = unsafe { *image.md }.size;
        if existing_shape != shape {
            return Err(RisioError::ShmExistsWithOtherShape {
                name: name.to_string(),
                existing: existing_shape,
                requested: shape,
            });
        }

        let existing_type: DataType = unsafe { *image.md }.datatype.try_into()?;
        if existing_type != T::get_data_type() {
            return Err(RisioError::ShmExistsWithOtherType {
                name: name.to_string(),
                existing: existing_type,
                requested: T::get_data_type(),
            });
        }

        Ok(Self {
            image,
            _data_type: PhantomData,
            shape,
        })
    }

    // pub fn ImageStreamIO_closeIm(image: *mut IMAGE) -> errno_t;

    pub fn sempost(&mut self, index: i64) -> Result<(), RisioError> {
        let err = unsafe { bindings::ImageStreamIO_sempost(&mut self.image, index) };
        RisioError::errno_to_error(err as i32)
    }

    pub fn array(&mut self) -> &mut [T] {
        unsafe { T::access_array(self) }
    }

    pub unsafe fn create_image(
        name: &str,
        naxis: i64,
        size: [u32; 3],
        location: i8,
        shared: bool,
        nb_sem: i32,
        nb_kw: i32,
        image_type: ImageType,
        cb_size: u32,
    ) -> Result<Image<T>, RisioError> {
        let mut size_internal: Vec<u32> = size.into();
        let mut image = std::mem::MaybeUninit::uninit();
        let name_c = CString::new(name)?;
        let err = unsafe {
            bindings::ImageStreamIO_createIm_gpu(
                image.as_mut_ptr(),
                name_c.as_ptr(),
                naxis,
                size_internal.as_mut_ptr(),
                T::get_data_type() as u8,
                location,
                shared as i32,
                nb_sem,
                nb_kw,
                u64::from(image_type),
                cb_size,
            )
        };
        RisioError::errno_to_error(err)?;
        let image = unsafe { image.assume_init() };

        Ok(Image {
            image,
            _data_type: PhantomData,
            shape: size,
        })
    }

    pub fn semwait(&mut self, index: i64) -> Result<(), RisioError> {
        let err = unsafe { bindings::ImageStreamIO_semwait(&mut self.image, index as i32) };
        RisioError::errno_to_error(err)
    }

    // pub fn ImageStreamIO_sempost_excl(
    //     image: *mut IMAGE,
    //     index: ::std::os::raw::c_long,
    // ) -> ::std::os::raw::c_long;

    // pub fn ImageStreamIO_sempost_loop(
    //     image: *mut IMAGE,
    //     index: ::std::os::raw::c_long,
    //     dtus: ::std::os::raw::c_long,
    // ) -> ::std::os::raw::c_long;

    // pub fn ImageStreamIO_getsemwaitindex(
    //     image: *mut IMAGE,
    //     semindexdefault: ::std::os::raw::c_int,
    // ) -> ::std::os::raw::c_int;

    pub fn semtrywait(&mut self, index: i64) -> Result<Option<()>, RisioError> {
        match unsafe { bindings::ImageStreamIO_semtrywait(&mut self.image, index as i32) } {
            0 => Ok(Some(())),
            x if x < 0 => Ok(None),
            err => RisioError::errno_to_error(err).map(|_| Some(())),
        }
    }

    // pub fn ImageStreamIO_semtimedwait(
    //     image: *mut IMAGE,
    //     index: ::std::os::raw::c_int,
    //     semwts: *const timespec,
    // ) -> ::std::os::raw::c_int;

    pub fn semflush(&mut self, index: i64) -> Result<(), RisioError> {
        let err = unsafe { bindings::ImageStreamIO_semflush(&mut self.image, index) };
        RisioError::errno_to_error(err as i32)
    }

    // pub fn ImageStreamIO_semvalue(
    //     image: *mut IMAGE,
    //     index: ::std::os::raw::c_long,
    // ) -> ::std::os::raw::c_long;

    // pub fn ImageStreamIO_UpdateIm_atime(
    //     image: *mut IMAGE,
    //     atime: *mut timespec,
    // ) -> ::std::os::raw::c_long;

    // pub fn ImageStreamIO_UpdateIm(image: *mut IMAGE) -> ::std::os::raw::c_long;
}

pub trait ValidImageType<T> {
    fn get_data_type() -> DataType;
    unsafe fn access_array(image: &mut Image<T>) -> &mut [T];
}

impl ValidImageType<u8> for u8 {
    fn get_data_type() -> DataType {
        DataType::U8
    }
    unsafe fn access_array(image: &mut Image<u8>) -> &mut [u8] {
        let len: usize = image.shape.iter().product::<u32>() as usize;
        unsafe { core::slice::from_raw_parts_mut(image.image.array.UI8, len) }
    }
}

impl ValidImageType<i8> for i8 {
    fn get_data_type() -> DataType {
        DataType::I8
    }

    unsafe fn access_array(image: &mut Image<i8>) -> &mut [i8] {
        let len: usize = image.shape.iter().product::<u32>() as usize;
        unsafe { core::slice::from_raw_parts_mut(image.image.array.SI8, len) }
    }
}

impl ValidImageType<u16> for u16 {
    fn get_data_type() -> DataType {
        DataType::I8
    }

    unsafe fn access_array(image: &mut Image<u16>) -> &mut [u16] {
        let len: usize = image.shape.iter().product::<u32>() as usize;
        unsafe { core::slice::from_raw_parts_mut(image.image.array.UI16, len) }
    }
}

impl ValidImageType<i16> for i16 {
    fn get_data_type() -> DataType {
        DataType::I16
    }

    unsafe fn access_array(image: &mut Image<i16>) -> &mut [i16] {
        let len: usize = image.shape.iter().product::<u32>() as usize;
        unsafe { core::slice::from_raw_parts_mut(image.image.array.SI16, len) }
    }
}

impl ValidImageType<u32> for u32 {
    fn get_data_type() -> DataType {
        DataType::U32
    }

    unsafe fn access_array(image: &mut Image<u32>) -> &mut [u32] {
        let len: usize = image.shape.iter().product::<u32>() as usize;
        unsafe { core::slice::from_raw_parts_mut(image.image.array.UI32, len) }
    }
}

impl ValidImageType<i32> for i32 {
    fn get_data_type() -> DataType {
        DataType::I32
    }

    unsafe fn access_array(image: &mut Image<i32>) -> &mut [i32] {
        let len: usize = image.shape.iter().product::<u32>() as usize;
        unsafe { core::slice::from_raw_parts_mut(image.image.array.SI32, len) }
    }
}

impl ValidImageType<u64> for u64 {
    fn get_data_type() -> DataType {
        DataType::U64
    }

    unsafe fn access_array(image: &mut Image<u64>) -> &mut [u64] {
        let len: usize = image.shape.iter().product::<u32>() as usize;
        unsafe { core::slice::from_raw_parts_mut(image.image.array.UI64, len) }
    }
}

impl ValidImageType<i64> for i64 {
    fn get_data_type() -> DataType {
        DataType::I64
    }

    unsafe fn access_array(image: &mut Image<i64>) -> &mut [i64] {
        let len: usize = image.shape.iter().product::<u32>() as usize;
        unsafe { core::slice::from_raw_parts_mut(image.image.array.SI64, len) }
    }
}

impl ValidImageType<f32> for f32 {
    fn get_data_type() -> DataType {
        DataType::F32
    }

    unsafe fn access_array(image: &mut Image<f32>) -> &mut [f32] {
        let len: usize = image.shape.iter().product::<u32>() as usize;
        unsafe { core::slice::from_raw_parts_mut(image.image.array.F, len) }
    }
}

impl ValidImageType<f64> for f64 {
    fn get_data_type() -> DataType {
        DataType::F64
    }

    unsafe fn access_array(image: &mut Image<f64>) -> &mut [f64] {
        let len: usize = image.shape.iter().product::<u32>() as usize;
        unsafe { core::slice::from_raw_parts_mut(image.image.array.D, len) }
    }
}

