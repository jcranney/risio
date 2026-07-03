use crate::bindings::*;
use crate::error::Error;
use anyhow::Result;
use libc::{aligned_alloc, pid_t};
use memmap2::MmapMut;
use rkyv::to_bytes;
use std::ffi::c_void;
use std::fmt::Debug;
use std::ptr::{slice_from_raw_parts, slice_from_raw_parts_mut};
use std::slice::from_raw_parts;
use std::{path::PathBuf, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq)]
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

pub trait IsioDataType {
    fn to_datatype() -> DataType;

    fn from_bytes<T>(data: &[u8]) -> &[T] {
        let nelements = data.len() / size_of::<T>();
        if data.len() % size_of::<T>() != 0 {
            panic!();
        }
        unsafe { slice_from_raw_parts(data.as_ptr().cast(), nelements).as_ref_unchecked() }
    }

    fn from_bytes_mut<T>(data: &mut [u8]) -> &mut [T] {
        let nelements = data.len() / size_of::<T>();
        if data.len() % size_of::<T>() != 0 {
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

impl TryFrom<u8> for DataType {
    fn try_from(value: u8) -> std::prelude::v1::Result<Self, Self::Error> {
        Ok(match value {
            1 => Self::U8,  // uint8_t
            2 => Self::I8,  // int8_t
            3 => Self::U16,  // uint16_t
            4 => Self::I16,  // int16_t
            5 => Self::U32,  // uint32_t
            6 => Self::I32,  // int32_t
            7 => Self::U64,  // uint64_t
            8 => Self::I64,  // int64_t,
            9 => Self::F32,  // IEEE 754 single-precision binary floating-point format: binary32
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


const fn round_up_8(x: usize) -> usize {
    (x + 7) & !7
}

impl IMAGE {
    fn _name(name: &str) -> [i8; STRINGMAXLEN_IMAGE_NAME as usize] {
        let mut out = [0; STRINGMAXLEN_IMAGE_NAME as usize];
        for (out_i, in_i) in out.iter_mut().zip(name.as_bytes()) {
            *out_i = *in_i as i8;
        }
        // must be null terminated, so let's force that here to be extra safe
        *out.last_mut().unwrap() = 0;

        out
    }

    fn sm_pname(name: &str) -> Result<PathBuf> {
        Ok(PathBuf::from_str(&format!("/dev/shm/{name}.im.shm"))?)
    }

    fn version() -> [i8; 32] {
        let mut version = [0; 32];
        version
            .iter_mut()
            .zip(IMAGESTRUCT_VERSION)
            .for_each(|(a, b)| {
                *a = *b as i8;
            });
        version
    }

    fn fetch_io_err<T>() -> Result<T> {
        let x = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        Err(std::io::Error::from_raw_os_error(x))?
    }

    // fn to_mmap_mut(self) -> Result<MmapMut> {
    //     todo!()
    // }

    fn from_mmap_mut(mmap: &mut MmapMut) -> Result<Self> {
        // so now we want to populate the data in a new IMAGE from mmap.
        // I guess either the mmap data is contiguous, or the IMAGE data is
        // contiguous, not both. So perhaps the IMAGE data is all simply cloned
        // from the SHM, including pointers etc.s

        let mut idx = 0;

        // first up is image metadata.
        let len = round_up_8(size_of::<IMAGE_METADATA>());
        let md: *mut IMAGE_METADATA = mmap[idx..idx + len].as_mut_ptr().cast();
        idx += len;

        let md_tmp: IMAGE_METADATA = unsafe { md.read().clone() };

        // image array:
        let len = round_up_8(md_tmp.imdatamemsize as usize);
        let array: IMAGE__bindgen_ty_1 = IMAGE__bindgen_ty_1 {
            UI8: mmap[idx..idx + len].as_mut_ptr().cast(),
        };
        idx += len;

        let len = round_up_8(size_of::<IMAGE_KEYWORD>() * md_tmp.NBkw as usize);
        let kw: *mut IMAGE_KEYWORD = mmap[idx..idx + len].as_mut_ptr().cast();
        idx += len;

        let len = round_up_8(size_of::<SEMFILEDATA>() * md_tmp.sem as usize);
        let semfile: *mut SEMFILEDATA = mmap[idx..idx + len].as_mut_ptr().cast();
        idx += len;

        let len = round_up_8(size_of::<sem_t>());
        let semlog: *mut sem_t = mmap[idx..idx + len].as_mut_ptr().cast();
        idx += len;

        let len = round_up_8(size_of::<i32>() * md_tmp.sem as usize);
        let sem_read_pid: *mut pid_t = mmap[idx..idx + len].as_mut_ptr().cast();
        idx += len;

        let len = round_up_8(size_of::<i32>() * md_tmp.sem as usize);
        let sem_write_pid: *mut pid_t = mmap[idx..idx + len].as_mut_ptr().cast();
        idx += len;

        let len = round_up_8(size_of::<u32>() * md_tmp.sem as usize);
        let sem_ctrl: *mut u32 = mmap[idx..idx + len].as_mut_ptr().cast();
        idx += len;

        let len = round_up_8(size_of::<u32>() * md_tmp.sem as usize);
        let sem_status: *mut u32 = mmap[idx..idx + len].as_mut_ptr().cast();
        idx += len;

        let len = round_up_8(size_of::<STREAM_PROC_TRACE>() * IMAGE_NB_PROCTRACE as usize);
        let stream_proc_trace: *mut STREAM_PROC_TRACE = mmap[idx..idx + len].as_mut_ptr().cast();
        idx += len;

        let len = round_up_8(size_of::<timespec>() * md_tmp.size[2] as usize);
        let atimearray: *mut timespec = mmap[idx..idx + len].as_mut_ptr().cast();
        idx += len;

        let len = round_up_8(size_of::<timespec>() * md_tmp.size[2] as usize);
        let writetimearray: *mut timespec = mmap[idx..idx + len].as_mut_ptr().cast();
        idx += len;

        let len = round_up_8(size_of::<u64>() * md_tmp.size[2] as usize);
        let cntarray: *mut u64 = mmap[idx..idx + len].as_mut_ptr().cast();
        idx += len;

        let len = round_up_8(size_of::<CBFRAMEMD>() * md_tmp.CBsize as usize);
        let circ_buff_md: *mut CBFRAMEMD = mmap[idx..idx + len].as_mut_ptr().cast();
        idx += len;

        let len = round_up_8(md_tmp.imdatamemsize as usize * md_tmp.CBsize as usize);
        let cb_imdata: *mut c_void = mmap[idx..idx + len].as_mut_ptr().cast();
        idx += len;

        let semptr: *mut *mut sem_t = &mut semfile.cast();

        assert_eq!(idx, mmap.len());

        let image = IMAGE {
            name: md_tmp.name,
            used: 1,
            createcnt: 1,
            shmfd: -1,
            memsize: mmap.len() as u64,
            semlog: semlog,
            md: md,
            array: array,
            semptr: semptr,
            kw: kw,
            semfile: semfile,
            semReadPID: sem_read_pid,
            semWritePID: sem_write_pid,
            semctrl: sem_ctrl,
            semstatus: sem_status,
            streamproctrace: stream_proc_trace,
            flagarray: [0; 10].as_mut_ptr(),
            cntarray: cntarray,
            atimearray: atimearray,
            writetimearray: writetimearray,
            CircBuff_md: circ_buff_md,
            CBimdata: cb_imdata,
        };

        Ok(image)
    }

    pub fn create_new_image_from_scratch(
        name: &str,
        shape: &[usize],
        datatype: DataType,
        nb_kw: usize,
        imagetype: ImageType,
        cb_size: usize,
        // shared: bool,  (shared==true for now...)
        // int8_t location, (-1: CPU RAM for now)
    ) -> Result<(Self, MmapMut)> {
        let naxis: usize = shape.len();
        const NB_SEM: usize = IMAGE_NB_SEMAPHORE as usize;

        // for the rust interface, we asserting that all axes are non-zero in
        // dimsion.
        if shape.iter().any(|s| s == &0) {
            return Err(Error::InvalidShapeArray {
                shape: shape.to_vec(),
            }
            .into());
        }

        // create a new shape3 with constant size 3 (like ISIO format)
        let shape3d = match shape.len() {
            1 => [shape[0] as u32, 0, 0],
            2 => [shape[0] as u32, shape[1] as u32, 0],
            3 => [shape[0] as u32, shape[1] as u32, shape[2] as u32],
            _ => {
                return Err(Error::InvalidNaxis {
                    shape: shape.to_vec(),
                })?;
            }
        };

        // number of elements is the product of the (non-zero) dimensions
        let nelement = shape.iter().product::<usize>();

        // data memory size is the size of the type, by the number of elements
        let imdatamemsize = datatype.typesize() * nelement;

        if imagetype.circular_buffer && (naxis != 3) {
            return Err(Error::CircBuffDimsWrong(naxis))?;
        }

        let array_raw = IMAGE__bindgen_ty_1 {
            raw: unsafe { aligned_alloc(8, imdatamemsize) },
        };

        let nbproctrace = IMAGE_NB_PROCTRACE as usize;

        let mut kw: Vec<IMAGE_KEYWORD> = (0..nb_kw).map(|_| IMAGE_KEYWORD::new()).collect();

        // - Assign pointers; initialize the semphores and their data
        let mut semptr: Vec<*mut sem_t> = vec![];
        let mut sem_read_pid: Vec<i32> = vec![];
        let mut sem_write_pid: Vec<i32> = vec![];
        let mut sem_ctrl: Vec<u32> = vec![];
        let mut sem_status: Vec<u32> = vec![];
        let mut semfile: Vec<SEMFILEDATA> = vec![];
        // TODO:, I'd like to look into how much of this is replaceable with
        // rust safe code.
        for semindex in 0..NB_SEM {
            let mut semfile_tmp: SEMFILEDATA = SEMFILEDATA {
                semdata: unsafe { std::mem::zeroed() },
            };
            match unsafe { sem_init(&mut semfile_tmp.semdata, 1, SEMAPHORE_INITVAL) } {
                e if e < 0 => Self::fetch_io_err()?,
                _ => (),
            };
            sem_read_pid.push(-1);
            sem_write_pid.push(-1);
            sem_ctrl.push(0);
            sem_status.push(0);
            semfile.push(semfile_tmp);
            semptr.push(&mut semfile[semindex].semdata);
        }

        let mut semlog = sem_t { __size: [0; 32] };
        match unsafe { sem_init(&mut semlog, 1, SEMAPHORE_INITVAL) } {
            e if e < 0 => Self::fetch_io_err()?,
            _ => (),
        };

        let mut stream_proc_trace: Vec<STREAM_PROC_TRACE> = vec![];
        stream_proc_trace.resize(IMAGE_NB_PROCTRACE as usize, STREAM_PROC_TRACE::new());

        let mut atimearray: Vec<timespec> = Vec::new();
        let mut writetimearray: Vec<timespec> = Vec::new();
        let mut cntarray: Vec<u64> = Vec::new();

        let len_timedim: usize = shape3d[2] as usize;
        // REMOVING THIS CHECK, TO BE CONSISTENT WITH C FUNCTION
        // match imagetype {
        //     ImageType {
        //         circular_buffer: true,
        //         ..
        //     }
        //     | ImageType {
        //         axis_encoding_code: ZAxisEncodingCode::TemporalCoordinate,
        //         ..
        //     } => {
        atimearray.resize(len_timedim, timespec::new());
        writetimearray.resize(len_timedim, timespec::new());
        cntarray.resize(len_timedim, 0);
        //     }
        //     _ => (),
        // };

        let mut circ_buff_md = Vec::new();
        circ_buff_md.resize(cb_size, CBFRAMEMD::new());

        let cb_imdata = unsafe { libc::malloc(imdatamemsize * cb_size) };

        let mut image_memsize: usize = round_up_8(size_of::<IMAGE_METADATA>());
        image_memsize += round_up_8(imdatamemsize);
        image_memsize += round_up_8(size_of::<IMAGE_KEYWORD>() * nb_kw);
        image_memsize += round_up_8(size_of::<SEMFILEDATA>() * NB_SEM);
        image_memsize += round_up_8(size_of::<sem_t>()); // semlog
        image_memsize += round_up_8(size_of::<i32>() * NB_SEM); // semReadPID
        image_memsize += round_up_8(size_of::<i32>() * NB_SEM); // semWritePID
        image_memsize += round_up_8(size_of::<u32>() * NB_SEM); // semctrl
        image_memsize += round_up_8(size_of::<u32>() * NB_SEM); // semstatus
        image_memsize += round_up_8(size_of::<STREAM_PROC_TRACE>() * nbproctrace);
        image_memsize += round_up_8(size_of::<timespec>() * len_timedim); // atimearray
        image_memsize += round_up_8(size_of::<timespec>() * len_timedim); // writetimearray
        image_memsize += round_up_8(size_of::<u64>() * len_timedim); // cntarray
        image_memsize += round_up_8(size_of::<CBFRAMEMD>() * cb_size);
        image_memsize += round_up_8(imdatamemsize * cb_size);

        unsafe { libc::umask(0) }; // TODO: I have no idea why this umask is needed

        let file_stat: libc::stat = unsafe { std::mem::zeroed() };
        // match unsafe { libc::fstat(fd, &mut file_stat) } {
        //     e if e < 0 => {
        //         unsafe { libc::close(fd) };
        //         Self::fetch_err()?;
        //     }
        //     0 => (),
        //     _ => unreachable!(),
        // };

        let mut last_access_time = timespec::new();
        unsafe { clock_gettime(CLOCK_TAI as i32, &mut last_access_time) };
        let mut creation_time = timespec::new();
        unsafe { clock_gettime(CLOCK_TAI as i32, &mut creation_time) };

        // let flagarray: [u64; 10] = [0; 10]; // TODO: This isn't initialised in ISIO, but worse
        // than that, there's no memory allocated for it in the map so accessing it will probably
        // segfault.

        let md = IMAGE_METADATA {
            version: Self::version(),
            name: Self::_name(name),
            naxis: naxis as u8,
            size: shape3d,
            nelement: nelement as u64,
            datatype: datatype as u8,
            imagetype: imagetype.into(),
            creationtime: creation_time,
            lastaccesstime: last_access_time,
            atime: timespec::new(),
            writetime: timespec::new(),
            creatorPID: unsafe { libc::getpid() },
            ownerPID: 0,
            shared: 1,
            inode: file_stat.st_ino,
            location: -1,
            status: 0,  // TODO: find initialisastion in C library.
            flag: 0,    // TODO: find initialisastion in C library.
            logflag: 0, // TODO: find initialisastion in C library.
            sem: NB_SEM as u16,
            NBproctrace: nbproctrace as u16,
            cnt0: 0,
            cnt1: 0,
            cnt2: 0, // TODO: find initialisastion in C library.
            write: 0,
            NBkw: nb_kw as u16,
            CBsize: cb_size as u32,
            CBindex: 0,
            CBcycle: 0,
            imdatamemsize: imdatamemsize as u64,
            cudaMemHandle: [0; 64], // TODO: find initialisastion in C library.
        };

        struct MapOwner {
            map: MmapMut,
            idx: usize,
        }
        impl MapOwner {
            fn new(file: std::fs::File) -> Result<Self> {
                Ok(Self {
                    map: unsafe { MmapMut::map_mut(&file)? },
                    idx: 0,
                })
            }
            fn get_next_mut_ptr(&mut self, len: usize) -> Result<&mut [u8]> {
                let new_idx = self.idx + len;
                if new_idx > self.map.len() {
                    return Err(Error::RequestingPointerBeyondRange {
                        map_len: self.map.len(),
                        requested: new_idx,
                    })?;
                }
                let result = Ok(&mut self.map[self.idx..self.idx + len]);
                self.idx = new_idx;
                result
            }
        }

        let file = std::fs::File::create_new(Self::sm_pname(name)?)?;
        file.set_len(image_memsize as u64)?;

        let mut mmap = MapOwner::new(file)?;

        mmap.get_next_mut_ptr(round_up_8(size_of::<IMAGE_METADATA>()))?
            .copy_from_slice(&mut to_bytes::<rkyv::rancor::Error>(&md).unwrap());

        mmap.get_next_mut_ptr(round_up_8(imdatamemsize))?
            .copy_from_slice(unsafe { from_raw_parts(array_raw.UI8, imdatamemsize) });

        mmap.get_next_mut_ptr(round_up_8(size_of::<IMAGE_KEYWORD>() * nb_kw))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts_mut(
                    kw.as_mut_ptr().cast(),
                    nb_kw * size_of::<IMAGE_KEYWORD>(),
                )
            });

        mmap.get_next_mut_ptr(round_up_8(size_of::<SEMFILEDATA>() * NB_SEM))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts_mut(
                    semfile.as_mut_ptr().cast(),
                    NB_SEM * size_of::<SEMFILEDATA>(),
                )
            });

        mmap.get_next_mut_ptr(round_up_8(size_of::<sem_t>()))?
            .copy_from_slice(&mut unsafe { semlog.__size.map(|x| x as u8) });

        mmap.get_next_mut_ptr(round_up_8(size_of::<i32>() * NB_SEM))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts_mut(
                    sem_read_pid.as_mut_ptr().cast(),
                    size_of::<i32>() * NB_SEM,
                )
            });
        mmap.get_next_mut_ptr(round_up_8(size_of::<i32>() * NB_SEM))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts_mut(
                    sem_write_pid.as_mut_ptr().cast(),
                    size_of::<i32>() * NB_SEM,
                )
            });
        mmap.get_next_mut_ptr(round_up_8(size_of::<u32>() * NB_SEM))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts_mut(
                    sem_ctrl.as_mut_ptr().cast(),
                    size_of::<u32>() * NB_SEM,
                )
            });
        mmap.get_next_mut_ptr(round_up_8(size_of::<u32>() * NB_SEM))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts_mut(
                    sem_write_pid.as_mut_ptr().cast(),
                    size_of::<u32>() * NB_SEM,
                )
            });
        mmap.get_next_mut_ptr(round_up_8(size_of::<STREAM_PROC_TRACE>() * nbproctrace))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts_mut(
                    stream_proc_trace.as_mut_ptr().cast(),
                    size_of::<STREAM_PROC_TRACE>() * nbproctrace,
                )
            });
        mmap.get_next_mut_ptr(round_up_8(size_of::<timespec>() * len_timedim))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts_mut(
                    atimearray.as_mut_ptr().cast(),
                    size_of::<timespec>() * len_timedim,
                )
            });
        mmap.get_next_mut_ptr(round_up_8(size_of::<timespec>() * len_timedim))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts_mut(
                    writetimearray.as_mut_ptr().cast(),
                    size_of::<timespec>() * len_timedim,
                )
            });
        mmap.get_next_mut_ptr(round_up_8(size_of::<u64>() * len_timedim))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts_mut(
                    cntarray.as_mut_ptr().cast(),
                    size_of::<u64>() * len_timedim,
                )
            });

        mmap.get_next_mut_ptr(round_up_8(size_of::<CBFRAMEMD>() * cb_size))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts_mut(
                    circ_buff_md.as_mut_ptr().cast(),
                    size_of::<CBFRAMEMD>() * cb_size,
                )
            });
        mmap.get_next_mut_ptr(round_up_8(imdatamemsize * cb_size))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts_mut(cb_imdata.cast(), imdatamemsize * cb_size)
            });
        assert_eq!(
            image_memsize, mmap.idx,
            "unexpected data size, something is wrong."
        );
        mmap.map.flush()?;

        let image = Self::from_mmap_mut(&mut mmap.map)?;
        Ok((image, mmap.map))
    }

    pub fn open_image(name: &str) -> Result<(Self, MmapMut)> {
        let file = std::fs::File::options()
            .read(true)
            .write(true)
            .open(Self::sm_pname(name)?)?;

        let mut mmap = unsafe { MmapMut::map_mut(&file) }?;

        let image = Self::from_mmap_mut(&mut mmap)?;
        Ok((image, mmap))
    }
}
