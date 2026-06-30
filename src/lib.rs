#[allow(
    non_snake_case,
    unnecessary_transmutes,
    non_upper_case_globals,
    non_camel_case_types
)]
pub mod bindings;
pub mod imagestreamio;
pub mod python;
use bindings::*;
use enum_iterator::{Sequence, all};
use std::ffi::NulError;
use std::fmt::Debug;
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

impl From<NulError> for RisioError {
    fn from(value: NulError) -> Self {
        RisioError::InvalidName { err: value }
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

#[derive(Debug, Clone, Copy)]
pub struct ImageType {
    pub circular_buffer: bool,
    pub vector_or_matrix: bool,
    pub stream_from_other_computer: bool,
    pub stream_for_other_computer: bool,
    pub axis_encoding_code: ZAxisEncodingCode,
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

impl ImageType {
    pub fn image() -> Self {
        Self {
            circular_buffer: false,
            vector_or_matrix: true,
            stream_from_other_computer: false,
            stream_for_other_computer: false,
            axis_encoding_code: ZAxisEncodingCode::default(),
        }
    }
    pub fn circular_buffer() -> Self {
        Self {
            circular_buffer: true,
            vector_or_matrix: true,
            stream_from_other_computer: false,
            stream_for_other_computer: false,
            axis_encoding_code: ZAxisEncodingCode::TemporalCoordinate,
        }
    }
}
