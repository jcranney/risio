use crate::bindings::*;
use crate::error::Error;
use std::fmt::Debug;
use std::ptr::{slice_from_raw_parts, slice_from_raw_parts_mut};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DataType {
    U8,  // uint8_t
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

impl Into<u8> for DataType {
    fn into(self) -> u8 {
        match self {
            DataType::U8 => 1,
            DataType::I8 => 2,
            DataType::U16 => 3,
            DataType::I16 => 4,
            DataType::U32 => 5,
            DataType::I32 => 6,
            DataType::U64 => 7,
            DataType::I64 => 8,
            DataType::F32 => 9,
            DataType::F64 => 10,
            DataType::C64 => 11,
            DataType::C128 => 12,
            DataType::F16 => 13,
        }
    }
}

pub trait IsioDataType {
    fn to_datatype() -> DataType;

    fn from_bytes<T>(data: &[u8]) -> &[T] {
        let nelements = data.len() / size_of::<T>();
        if !data.len().is_multiple_of(size_of::<T>()) {
            panic!();
        }
        unsafe { slice_from_raw_parts(data.as_ptr().cast(), nelements).as_ref_unchecked() }
    }

    fn from_bytes_mut<T>(data: &mut [u8]) -> &mut [T] {
        let nelements = data.len() / size_of::<T>();
        if !data.len().is_multiple_of(size_of::<T>()) {
            panic!();
        }
        unsafe { slice_from_raw_parts_mut(data.as_mut_ptr().cast(), nelements).as_mut_unchecked() }
    }
}

impl IsioDataType for u8 {
    fn to_datatype() -> DataType {
        DataType::U8
    }
}
impl IsioDataType for u16 {
    fn to_datatype() -> DataType {
        DataType::U16
    }
}
impl IsioDataType for u32 {
    fn to_datatype() -> DataType {
        DataType::U32
    }
}
impl IsioDataType for u64 {
    fn to_datatype() -> DataType {
        DataType::U64
    }
}
impl IsioDataType for i8 {
    fn to_datatype() -> DataType {
        DataType::I8
    }
}
impl IsioDataType for i16 {
    fn to_datatype() -> DataType {
        DataType::I16
    }
}
impl IsioDataType for i32 {
    fn to_datatype() -> DataType {
        DataType::I32
    }
}
impl IsioDataType for i64 {
    fn to_datatype() -> DataType {
        DataType::I64
    }
}
impl IsioDataType for f32 {
    fn to_datatype() -> DataType {
        DataType::F32
    }
}
impl IsioDataType for f64 {
    fn to_datatype() -> DataType {
        DataType::F64
    }
}

impl DataType {
    pub fn typesize(&self) -> usize {
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

impl TryFrom<u8> for DataType {
    fn try_from(value: u8) -> std::prelude::v1::Result<Self, Self::Error> {
        Ok(match value {
            1 => Self::U8,    // uint8_t
            2 => Self::I8,    // int8_t
            3 => Self::U16,   // uint16_t
            4 => Self::I16,   // int16_t
            5 => Self::U32,   // uint32_t
            6 => Self::I32,   // int32_t
            7 => Self::U64,   // uint64_t
            8 => Self::I64,   // int64_t,
            9 => Self::F32,   // IEEE 754 single-precision binary floating-point format: binary32
            10 => Self::F64,  // IEEE 754 double-precision binary floating-point format: binary64
            11 => Self::C64,  // complex_float
            12 => Self::C128, // complex double
            13 => Self::F16,  // half precision floating-point
            x => return Err(Error::UnsupportedDataType(x)),
        })
    }

    type Error = Error;
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
