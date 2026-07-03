use crate::DataType;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Illegal number of axes ({}) for requested shape: {:?}. \
    Valid images must be 1D, 2D, or 3D.", shape.len(), shape)]
    InvalidNaxis { shape: Vec<usize> },
    #[error("Circular buffer must have 3 dimensions, but found naxis={0}")]
    CircBuffDimsWrong(usize),
    #[error("Shape array is invalid; cannot contain zeros: {:?}", shape)]
    InvalidShapeArray { shape: Vec<usize> },
    #[error(
        "Requested a slice from memory map which exceeds the size of the map. \
        Map length: {map_len}, requested up to idx: {requested}"
    )]
    RequestingPointerBeyondRange { map_len: usize, requested: usize },
    #[error("Expected IMAGE with type: {:?}, but found image with type: {:?}", expected, found)]
    MismatchDataType{expected: DataType, found: DataType},
    #[error("Unsupported data type index: {0}")]
    UnsupportedDataType(u8),
}