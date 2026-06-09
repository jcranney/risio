pub mod bindings;
use crate::bindings::{IMAGE, IMGID, errno_t};
use anyhow::Result;
use std::ffi::CString;
use std::fmt::Debug;
use std::marker::PhantomData;
use thiserror::Error;


#[derive(Error, Debug)]
pub enum RisioError {
    #[error("Core ImageStreamIO library error: {0}")]
    ImageStreamIOError(i32),
}

#[derive(Debug, Clone, Copy)]
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
/// Todo: check with Olivier if the encoding code is intended to represent other
/// axes than the zeroth too.
#[derive(Debug, Clone, Copy)]
pub struct ImageType {
    circular_buffer: bool,
    vector_or_matrix: bool,
    stream_from_other_computer: bool,
    stream_for_other_computer: bool,
    axis_encoding_code: Axis0EncodingCode,
}

#[derive(Debug, Clone, Copy)]
pub enum Axis0EncodingCode {
    Undefined,
    SpatialCoordinate,
    TemporalCoordinate,
    WavelengthCoordinate,
    MappingIndex,
}

impl Default for Axis0EncodingCode {
    fn default() -> Self {
        Axis0EncodingCode::Undefined
    }
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
            Axis0EncodingCode::Undefined => 0x0 << 16,
            Axis0EncodingCode::SpatialCoordinate => 0x1 << 16,
            Axis0EncodingCode::TemporalCoordinate => 0x2 << 16,
            Axis0EncodingCode::WavelengthCoordinate => 0x3 << 16,
            Axis0EncodingCode::MappingIndex => 0x4 << 16,
        };
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
            axis_encoding_code: Axis0EncodingCode::default(),
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
    shape: Vec<u32>,
}

impl<T> Image<T> {
    pub fn destroy_im(mut self) -> Result<()> {
        match unsafe { bindings::ImageStreamIO_destroyIm(&mut self.image) } {
            0 => Ok(()),
            err => Err(RisioError::ImageStreamIOError(err).into()),
        }
    }

    // pub fn ImageStreamIO_openIm(image: *mut IMAGE, name: *const ::std::os::raw::c_char) -> errno_t;

    // pub fn ImageStreamIO_get_image_d_ptr(image: *mut IMAGE) -> *mut ::std::os::raw::c_void;

    /// todo: extract shape from IMAGE object
    pub fn read_sharedmem_image(name: &str, shape: &[u32]) -> Result<Self> {
        let name_c = CString::new(name)?;
        let mut image = std::mem::MaybeUninit::uninit();

        unsafe {
            bindings::ImageStreamIO_read_sharedmem_image_toIMAGE(
                name_c.as_ptr(),
                image.as_mut_ptr(),
            )
        };
        let image = unsafe { image.assume_init() };
        Ok(Self {
            image,
            _data_type: PhantomData,
            shape: shape.into(),
        })
    }

    // pub fn ImageStreamIO_closeIm(image: *mut IMAGE) -> errno_t;

    pub fn sempost(&mut self, index: i64) -> i64 {
        unsafe { bindings::ImageStreamIO_sempost(&mut self.image, index) }
    }
    // pub fn ImageStreamIO_sempost(
    //     image: *mut IMAGE,
    //     index: ::std::os::raw::c_long,
    // ) -> ::std::os::raw::c_long;

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

    // pub fn ImageStreamIO_semwait(
    //     image: *mut IMAGE,
    //     index: ::std::os::raw::c_int,
    // ) -> ::std::os::raw::c_int;

    // pub fn ImageStreamIO_semtrywait(
    //     image: *mut IMAGE,
    //     index: ::std::os::raw::c_int,
    // ) -> ::std::os::raw::c_int;

    // pub fn ImageStreamIO_semtimedwait(
    //     image: *mut IMAGE,
    //     index: ::std::os::raw::c_int,
    //     semwts: *const timespec,
    // ) -> ::std::os::raw::c_int;

    // pub fn ImageStreamIO_semflush(
    //     image: *mut IMAGE,
    //     index: ::std::os::raw::c_long,
    // ) -> ::std::os::raw::c_long;

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

pub trait ValidImage<T> {
    fn get_data_type() -> DataType;

    fn array(&self) -> &mut [T];

    fn create_image(
        name: &str,
        naxis: i64,
        size: &[u32],
        location: i8,
        shared: bool,
        nb_sem: i32,
        nb_kw: i32,
        image_type: ImageType,
        cb_size: u32,
    ) -> Result<Image<T>> {
        let mut size_internal: Vec<u32> = size.into();
        let mut image = std::mem::MaybeUninit::uninit();
        let name_c = CString::new(name)?;
        let _err = unsafe {
            bindings::ImageStreamIO_createIm_gpu(
                image.as_mut_ptr(),
                name_c.as_ptr(),
                naxis,
                size_internal.as_mut_ptr(),
                Self::get_data_type() as u8,
                location,
                shared as i32,
                nb_sem,
                nb_kw,
                u64::from(image_type),
                cb_size,
            );
        };
        let image = unsafe { image.assume_init() };

        Ok(Image {
            image,
            _data_type: PhantomData,
            shape: size.into(),
        })
    }
}

impl ValidImage<u8> for Image<u8> {
    fn get_data_type() -> DataType {
        DataType::U8
    }

    fn array(&self) -> &mut [u8] {
        let len: usize = self.shape.iter().product::<u32>() as usize;
        unsafe { core::slice::from_raw_parts_mut(self.image.array.UI8, len) }
    }
}

impl ValidImage<i8> for Image<i8> {
    fn get_data_type() -> DataType {
        DataType::I8
    }

    fn array(&self) -> &mut [i8] {
        let len: usize = self.shape.iter().product::<u32>() as usize;
        unsafe { core::slice::from_raw_parts_mut(self.image.array.SI8, len) }
    }
}
impl ValidImage<u16> for Image<u16> {
    fn get_data_type() -> DataType {
        DataType::U16
    }

    fn array(&self) -> &mut [u16] {
        let len: usize = self.shape.iter().product::<u32>() as usize;
        unsafe { core::slice::from_raw_parts_mut(self.image.array.UI16, len) }
    }
}
impl ValidImage<i16> for Image<i16> {
    fn get_data_type() -> DataType {
        DataType::I16
    }

    fn array(&self) -> &mut [i16] {
        let len: usize = self.shape.iter().product::<u32>() as usize;
        unsafe { core::slice::from_raw_parts_mut(self.image.array.SI16, len) }
    }
}
impl ValidImage<u32> for Image<u32> {
    fn get_data_type() -> DataType {
        DataType::U32
    }

    fn array(&self) -> &mut [u32] {
        let len: usize = self.shape.iter().product::<u32>() as usize;
        unsafe { core::slice::from_raw_parts_mut(self.image.array.UI32, len) }
    }
}
impl ValidImage<i32> for Image<i32> {
    fn get_data_type() -> DataType {
        DataType::I32
    }

    fn array(&self) -> &mut [i32] {
        let len: usize = self.shape.iter().product::<u32>() as usize;
        unsafe { core::slice::from_raw_parts_mut(self.image.array.SI32, len) }
    }
}
impl ValidImage<u64> for Image<u64> {
    fn get_data_type() -> DataType {
        DataType::U64
    }

    fn array(&self) -> &mut [u64] {
        let len: usize = self.shape.iter().product::<u32>() as usize;
        unsafe { core::slice::from_raw_parts_mut(self.image.array.UI64, len) }
    }
}
impl ValidImage<i64> for Image<i64> {
    fn get_data_type() -> DataType {
        DataType::I64
    }

    fn array(&self) -> &mut [i64] {
        let len: usize = self.shape.iter().product::<u32>() as usize;
        unsafe { core::slice::from_raw_parts_mut(self.image.array.SI64, len) }
    }
}
impl ValidImage<f32> for Image<f32> {
    fn get_data_type() -> DataType {
        DataType::F32
    }

    fn array(&self) -> &mut [f32] {
        let len: usize = self.shape.iter().product::<u32>() as usize;
        unsafe { core::slice::from_raw_parts_mut(self.image.array.F, len) }
    }
}
impl ValidImage<f64> for Image<f64> {
    fn get_data_type() -> DataType {
        DataType::F64
    }

    fn array(&self) -> &mut [f64] {
        let len: usize = self.shape.iter().product::<u32>() as usize;
        unsafe { core::slice::from_raw_parts_mut(self.image.array.D, len) }
    }
}
impl ValidImage<num_complex::Complex32> for Image<num_complex::Complex32> {
    fn get_data_type() -> DataType {
        DataType::C64
    }

    fn array(&self) -> &mut [num_complex::Complex32] {
        unimplemented!()
    }
}
impl ValidImage<num_complex::Complex64> for Image<num_complex::Complex64> {
    fn get_data_type() -> DataType {
        DataType::C128
    }

    fn array(&self) -> &mut [num_complex::Complex64] {
        unimplemented!()
    }
}

impl bindings::IMGID {
    pub fn array_f64(&mut self) -> &mut [f64] {
        unsafe { core::slice::from_raw_parts_mut((*self.im).array.D, 10) }
    }

    fn create(name: &str, shape: Vec<usize>, shared: bool, datatype: DataType) -> Result<Self> {
        let name_c = &mut [0; bindings::STRINGMAXLEN_IMAGE_NAME as usize];
        for (i, char_i) in name.chars().enumerate() {
            name_c[i] = char_i as i8;
        }

        let mut img = IMGID {
            ID: -1,
            createcnt: -1,
            name: *name_c,
            im: std::ptr::null_mut(),
            md: std::ptr::null_mut(),
            datatype: datatype as u8,
            naxis: shape.len() as i32,
            size: [shape[0] as u32, shape[1] as u32, shape[2] as u32],
            shared: shared as i32,
            NBkw: bindings::NB_KEYWNODE_MAX as i32,
            CBsize: 0,
        };
        unsafe { img.resolve_img_id(ErrMode::WARN) };
        Ok(img)
    }

    /** @brief Resolve image already in memory
     *
     *
     *
     * ERRMODE values
     * ERRMODE_WARN : print warning
     * ERRMODE_FAIL : error
     * ERRMODE_ABORT : abort
     */
    unsafe fn resolve_img_id(&mut self, errmode: ErrMode) -> bindings::imageID { unsafe {
        // IF:
        // Not resolved before OR create counter mismatch OR not used.
        // Note: we are comparing img->createcnt to data.image[img->ID].createcnt to check if the
        // image has been re-created, indicating that our pointers are stale.
        let mut data_image = unsafe { *bindings::data.image.wrapping_add(self.ID as usize) };
        if (self.ID == -1) || (self.createcnt != data_image.createcnt) || (data_image.used != 1) {
            self.ID = unsafe { bindings::image_ID(self.name.as_ptr()) };
            if self.ID > -1
            // Resolve success !
            {
                self.im = &mut data_image;
                self.md = data_image.md;
                self.createcnt = data_image.createcnt;

                // Populate the IMGID from the imageID metadata
                self.update_img_id_creationparams();
            }
        }

        // if still unresolved
        //
        if self.ID == -1 {
            match errmode {
                ErrMode::FAIL | ErrMode::ABORT => {
                    eprintln!("Cannot resolve image {:?}\n", self.name);
                    unsafe { bindings::abort() };
                }
                ErrMode::WARN => {
                    eprintln!("Cannot resolve image {:?}\n", self.name);
                }
                ErrMode::NULL => (),
            }
        }

        return self.ID;
    }}

    unsafe fn update_img_id_creationparams(&mut self) -> bindings::errno_t {
        unsafe {
            self.datatype = (*self.md).datatype;
            self.naxis = (*self.md).naxis as i32;
            for ii in 0..3 {
                self.size[ii] = (*self.md).size[ii];
            }
            self.shared = (*self.md).shared as i32;
            self.NBkw = (*self.md).NBkw as i32;
            self.CBsize = (*self.md).CBsize as i32;
        }

        return bindings::RETURN_SUCCESS as i32;
    }

    pub unsafe fn stream_connect_create_2D(
        name: &str,
        xsize: usize,
        ysize: usize,
        datatype: DataType,
    ) -> Result<Self> { unsafe {
        let mut img: Self = Self::create(name, vec![xsize, ysize, 0], true, datatype)?;
        unsafe { img.resolve_img_id(ErrMode::WARN) };

        if img.ID == -1 {
            // try to connect to shared memory if not in local memory already
            unsafe { bindings::read_sharedmem_image(img.name.as_ptr()) };
            unsafe { img.resolve_img_id(ErrMode::WARN) };
        }

        if img.ID != -1 {
            // if in local memory,
            // create blank img for comparison
            let mut imgc: bindings::IMGID = Self::make_blank();
            imgc.datatype = datatype as u8;
            imgc.naxis = 2;
            imgc.size[0] = xsize as u32;
            imgc.size[1] = ysize as u32;
            imgc.NBkw = bindings::NB_KEYWNODE_MAX as i32;
            let err = img.compare(&imgc);
            println!("{} errors", err);

            // if doesn't pass test, erase from local memory
            if err != 0 {
                unsafe {
                    bindings::delete_image_ID(
                        img.name.as_ptr(),
                        bindings::DELETE_IMAGE_ERRMODE_WARNING as i32,
                    )
                };
                img.ID = -1;
            }
        }

        // if not in local memory, (re)-create
        if img.ID == -1 {
            Self::create(name, vec![xsize, ysize], true, datatype)?;
        }

        if img.ID != -1 {
            let id: bindings::imageID = img.ID;
            img.im = unsafe { bindings::data.image.wrapping_add(id as usize) };
            img.md = (unsafe { *img.im }).md;
            img.createcnt = (unsafe { *img.im }).createcnt;
            img.update_creation_params();
        }

        return Ok(img);
    }}

    fn make_blank() -> Self {
        IMGID {
            ID: -1,
            createcnt: -1,
            name: [0; bindings::STRINGMAXLEN_IMAGE_NAME as usize],
            im: std::ptr::null_mut(),
            md: std::ptr::null_mut(),
            datatype: bindings::_DATATYPE_UNINITIALIZED as u8,
            naxis: -1,
            size: [0; 3],
            shared: -1,
            NBkw: -1,
            CBsize: -1,
        }
    }

    fn compare(&self, other: &Self) -> u64 {
        let mut comp_err = 0;

        if other.datatype != bindings::_DATATYPE_UNINITIALIZED as u8 {
            print!("Checking datatype       ");
            if other.datatype != self.datatype {
                println!("FAIL");
                comp_err += 1;
            } else {
                println!("PASS");
            }
        }

        if other.naxis != -1 {
            print!("Checking naxis  {} {}    ", other.naxis, self.naxis);
            if other.naxis != self.naxis  {
                println!("FAIL");
                comp_err += 1;
            } else {
                println!("PASS");
            }
        }

        if other.size[0] != 0 {
            print!("Checking size[0]        ");
            if other.size[0] != self.size[0]  {
                println!("FAIL");
                comp_err += 1;
            } else {
                println!("PASS");
            }
        }

        if other.size[1] != 0 {
            print!("Checking size[1]        ");
            if other.size[1] != self.size[1]  {
                println!("FAIL");
                comp_err += 1;
            } else {
                println!("PASS");
            }
        }

        if other.size[2] != 0 {
            print!("Checking size[2]        ");
            if other.size[2] != self.size[2]  {
                println!("FAIL");
                comp_err += 1;
            } else {
                println!("PASS");
            }
        }

        print!("Checking NBkw           ");
        if other.NBkw != self.NBkw {
            println!("FAIL");
            print!("   {:4}  {:?}\n", other.NBkw, other.name);
            print!("   {:4}  {:?}\n", self.NBkw, self.name);
            comp_err += 1;
        } else {
            println!("PASS");
        }

        comp_err
    }

    unsafe fn update_creation_params(&mut self) -> errno_t {
        let md = unsafe { *self.md };
        self.datatype = md.datatype;
        self.naxis = md.naxis as i32;
        for ii in 0..3 {
            self.size[ii] = md.size[ii];
        }
        self.shared = md.shared as i32;
        self.NBkw = md.NBkw as i32;
        self.CBsize = md.CBsize as i32;

        bindings::RETURN_SUCCESS as i32
    }
}

pub enum ErrMode {
    NULL = 0,
    WARN,
    FAIL,
    ABORT,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_shm_image() {}
}
