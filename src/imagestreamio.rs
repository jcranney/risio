use crate::TimeSpec;
use crate::bindings::*;
use crate::datatype::*;
use crate::error::Error;
use crate::imagestreamio::byte_structs::*;
use anyhow::Result;
use memmap2::MmapMut;
use std::ffi::c_void;
use std::slice::from_raw_parts;
use std::slice::from_raw_parts_mut;
use std::{path::PathBuf, str::FromStr};

const fn round_up_8(x: usize) -> usize {
    (x + 7) & !7
}

impl<'a> Image<'a> {
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

    unsafe fn from_mmap_mut(mut mmap: MmapMut) -> Result<Self> {
        // so now we want to populate the data in a new IMAGE from mmap.
        // I guess either the mmap data is contiguous, or the IMAGE data is
        // contiguous, not both. So perhaps the IMAGE data is all simply cloned
        // from the SHM, including pointers etc.s
        todo!();
        let memsize = mmap.len();

        // first up is image metadata.
        let (chunk, leftover) = mmap.split_at_mut(round_up_8(size_of::<ImageMetadata>()));
        let md: &mut ImageMetadata = unsafe {
            chunk
                .as_mut_ptr()
                .cast::<ImageMetadata>()
                .as_mut_unchecked()
        };

        let md_tmp = md.clone();

        // image array:
        let (chunk, leftover) = leftover.split_at_mut(round_up_8(md_tmp.imdatamemsize as usize));
        let array = chunk;

        let (chunk, leftover) = leftover.split_at_mut(round_up_8(
            size_of::<ImageKeyword>() * md_tmp.nb_kw as usize,
        ));
        let kw: &mut [ImageKeyword] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.nb_kw as usize) };

        let (chunk, leftover) =
            leftover.split_at_mut(round_up_8(size_of::<SemFileData>() * md_tmp.sem as usize));
        let semfile: &mut [SemFileData] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.sem as usize) };

        let (chunk, leftover) = leftover.split_at_mut(round_up_8(size_of::<Sem>()));
        let semlog: &mut Sem = { unsafe { chunk.as_mut_ptr().cast::<Sem>().as_mut_unchecked() } };

        let (chunk, leftover) =
            leftover.split_at_mut(round_up_8(size_of::<i32>() * md_tmp.sem as usize));
        let sem_read_pid: &mut [i32] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.sem as usize) };

        let (chunk, leftover) =
            leftover.split_at_mut(round_up_8(size_of::<i32>() * md_tmp.sem as usize));
        let sem_write_pid: &mut [i32] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.sem as usize) };

        let (chunk, leftover) =
            leftover.split_at_mut(round_up_8(size_of::<u32>() * md_tmp.sem as usize));
        let sem_ctrl: &mut [u32] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.sem as usize) };

        let (chunk, leftover) =
            leftover.split_at_mut(round_up_8(size_of::<u32>() * md_tmp.sem as usize));
        let sem_status: &mut [u32] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.sem as usize) };

        let (chunk, leftover) = leftover.split_at_mut(round_up_8(
            size_of::<StreamProcTrace>() * IMAGE_NB_PROCTRACE as usize,
        ));
        let stream_proc_trace: &mut [StreamProcTrace] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), IMAGE_NB_PROCTRACE as usize) };

        let (chunk, leftover) =
            leftover.split_at_mut(round_up_8(size_of::<TimeSpec>() * md_tmp.size[2] as usize));
        let atimearray: &mut [TimeSpec] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.size[2] as usize) };

        let (chunk, leftover) =
            leftover.split_at_mut(round_up_8(size_of::<TimeSpec>() * md_tmp.size[2] as usize));
        let writetimearray: &mut [TimeSpec] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.size[2] as usize) };

        let (chunk, leftover) =
            leftover.split_at_mut(round_up_8(size_of::<u64>() * md_tmp.size[2] as usize));
        let cntarray: &mut [u64] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.size[2] as usize) };

        let (chunk, leftover) = leftover.split_at_mut(round_up_8(
            size_of::<CBFrameMetadata>() * md_tmp.cb_size as usize,
        ));
        let circ_buff_md: &mut [CBFrameMetadata] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.cb_size as usize) };

        let (chunk, leftover) = leftover.split_at_mut(round_up_8(
            md_tmp.imdatamemsize as usize * md_tmp.cb_size as usize,
        ));
        let cb_imdata: &mut [u8] = chunk;

        // let semptr: &mut [&mut Sem] = &mut semfile.iter_mut().map(|s| {
        //     let SemFileData { semdata } = s;
        //     semdata
        // }).collect();

        // should be nothing left in mmap:
        assert_eq!(leftover.len(), 0);

        let image = Self {
            name: md_tmp.name,
            used: 1,
            createcnt: 1,
            shmfd: -1,
            memsize: memsize as u64,
            semlog,
            md,
            array,
            // semptr,
            kw,
            semfile,
            sem_read_pid,
            sem_write_pid,
            semctrl: sem_ctrl,
            semstatus: sem_status,
            streamproctrace: stream_proc_trace,
            // flagarray: [].as_mut(),
            cntarray,
            atimearray,
            writetimearray,
            circ_buff_md,
            cb_imdata,
            mmap: None,
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
    ) -> Result<Self> {
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

        let mut array_init: Vec<u8> = vec![0; imdatamemsize];
        let array_raw = IMAGE__bindgen_ty_1 {
            raw: array_init.as_mut_ptr().cast(),
        };

        let nbproctrace = IMAGE_NB_PROCTRACE as usize;

        let kw: Vec<IMAGE_KEYWORD> = (0..nb_kw).map(|_| IMAGE_KEYWORD::new()).collect();

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

        let semlog: *mut sem_t = &mut unsafe { std::mem::zeroed() };
        match unsafe { sem_init(semlog, 1, SEMAPHORE_INITVAL) } {
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

        let cb_imdata: Vec<u8> = vec![0; imdatamemsize * cb_size];

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

        // let file_stat: stat = unsafe { std::mem::zeroed() };
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

        let md: *mut IMAGE_METADATA = &mut IMAGE_METADATA {
            version: Self::version(),
            name: Self::_name(name),
            naxis: naxis as u8,
            size: shape3d,
            nelement: nelement as u64,
            datatype: datatype.into(),
            imagetype: imagetype.into(),
            creationtime: creation_time,
            lastaccesstime: last_access_time,
            atime: timespec::new(),
            writetime: timespec::new(),
            creatorPID: std::process::id() as i32,
            ownerPID: 0,
            shared: 1,
            inode: 0, // file_stat.st_ino, TODO: learn why this is needed and set it properly
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
            .copy_from_slice(unsafe { from_raw_parts(md.cast(), size_of::<IMAGE_METADATA>()) });

        mmap.get_next_mut_ptr(round_up_8(imdatamemsize))?
            .copy_from_slice(unsafe { from_raw_parts(array_raw.UI8, imdatamemsize) });

        mmap.get_next_mut_ptr(round_up_8(size_of::<IMAGE_KEYWORD>() * nb_kw))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts(kw.as_ptr().cast(), nb_kw * size_of::<IMAGE_KEYWORD>())
            });

        mmap.get_next_mut_ptr(round_up_8(size_of::<SEMFILEDATA>() * NB_SEM))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts(
                    semfile.as_ptr().cast(),
                    NB_SEM * size_of::<SEMFILEDATA>(),
                )
            });

        mmap.get_next_mut_ptr(round_up_8(size_of::<sem_t>()))?
            .copy_from_slice(unsafe { from_raw_parts(semlog.cast(), size_of::<sem_t>()) });

        mmap.get_next_mut_ptr(round_up_8(size_of::<i32>() * NB_SEM))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts(sem_read_pid.as_ptr().cast(), size_of::<i32>() * NB_SEM)
            });
        mmap.get_next_mut_ptr(round_up_8(size_of::<i32>() * NB_SEM))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts(
                    sem_write_pid.as_ptr().cast(),
                    size_of::<i32>() * NB_SEM,
                )
            });
        mmap.get_next_mut_ptr(round_up_8(size_of::<u32>() * NB_SEM))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts(sem_ctrl.as_ptr().cast(), size_of::<u32>() * NB_SEM)
            });
        mmap.get_next_mut_ptr(round_up_8(size_of::<u32>() * NB_SEM))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts(
                    sem_write_pid.as_ptr().cast(),
                    size_of::<u32>() * NB_SEM,
                )
            });
        mmap.get_next_mut_ptr(round_up_8(size_of::<STREAM_PROC_TRACE>() * nbproctrace))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts(
                    stream_proc_trace.as_ptr().cast(),
                    size_of::<STREAM_PROC_TRACE>() * nbproctrace,
                )
            });
        mmap.get_next_mut_ptr(round_up_8(size_of::<timespec>() * len_timedim))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts(
                    atimearray.as_ptr().cast(),
                    size_of::<timespec>() * len_timedim,
                )
            });
        mmap.get_next_mut_ptr(round_up_8(size_of::<timespec>() * len_timedim))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts(
                    writetimearray.as_ptr().cast(),
                    size_of::<timespec>() * len_timedim,
                )
            });
        mmap.get_next_mut_ptr(round_up_8(size_of::<u64>() * len_timedim))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts(
                    cntarray.as_ptr().cast(),
                    size_of::<u64>() * len_timedim,
                )
            });

        mmap.get_next_mut_ptr(round_up_8(size_of::<CBFRAMEMD>() * cb_size))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts(
                    circ_buff_md.as_ptr().cast(),
                    size_of::<CBFRAMEMD>() * cb_size,
                )
            });
        mmap.get_next_mut_ptr(round_up_8(imdatamemsize * cb_size))?
            .copy_from_slice(unsafe {
                core::slice::from_raw_parts(cb_imdata.as_ptr().cast(), imdatamemsize * cb_size)
            });
        assert_eq!(
            image_memsize, mmap.idx,
            "unexpected data size, something is wrong."
        );
        mmap.map.flush()?;
        let mut map = mmap.map;
        let mut image = unsafe { Self::from_mmap_mut(map) }?;
        Ok(image)
    }

    pub fn open_image(name: &str) -> Result<Self> {
        let file = std::fs::File::options()
            .read(true)
            .write(true)
            .open(Self::sm_pname(name)?)?;

        let mut mmap = unsafe { MmapMut::map_mut(&file) }?;

        let image = unsafe { Self::from_mmap_mut(mmap) }?;
        Ok(image)
    }
}

pub mod byte_structs {
    //! All of the structs defined in this module have the exact same byte representation
    //! as their native C type, and can be directly transmuted from the bits in
    //! memory. It's not clear how to guarantee that the byte representation is
    //! the same, except through inspecting with the `bindgen` generated structs.
    //!
    //! Ensuring that the byte values are *valid* values is a different story, and
    //! that is beyond the scope of the struct implementation. It should always be
    //! possible to transmute from a sequence of bytes into these types.

    /// The ImageKeyword structure contains a name, value, count and comment,
    /// and a c_char valued discriminant with the legal values of:
    ///  - c'N', unused,
    ///  - c'L', long,
    ///  - c'D', double,
    ///  - c'S', 16-char string.
    #[repr(C)]
    pub struct ImageKeyword {
        pub name: [u8; 16],
        /// N: unused, L: long, D: double, S: 16-char string
        pub keyword_type: u8,
        pub value: KeywordValue,
        pub cnt: u64,
        pub comment: [u8; 80],
    }

    /// Keyword value - in order to reference directly from memory we keep a
    /// union representation, despite that being unsafe and annoying.
    #[repr(C)]
    pub union KeywordValue {
        numl: i64,
        numf: f64,
        valstr: [u8; 16],
    }

    /// StreamProcTrace holds trigger and timing info. Array of StreamProcTrace
    /// is held within streams to track history. This information is assembled
    /// by a process, and then written to all streams it writes.
    #[repr(C)]
    #[derive(Debug, Clone)]
    pub struct StreamProcTrace {
        triggermode: i32,
        /// PID of process writing stream. 0 if no entry
        procwrite_pid: i32,
        /// trigger stream inode
        trigger_inode: i64,
        /// timestamp process triggered
        ts_procstart: TimeSpec,
        /// timestamp write this stream
        ts_streamupdate: TimeSpec,
        /// trigger semaphore
        trigsemindex: i32,
        triggerstatus: i32,
        /// trigger stream cnt0 value at trigger
        cnt0: u64,
    }

    /// Circular Buffer Frame Metadata
    #[repr(C)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct CBFrameMetadata {
        cnt0: u64,
        cnt1: u64,
        atime: TimeSpec,
        writetime: TimeSpec,
    }

    /// TimeSpec, used for any "instant" style timestamp.
    #[repr(C)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct TimeSpec {
        pub tv_sec: u64,
        pub tv_nsec: u64,
    }

    /// Image metadata
    #[repr(C)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct ImageMetadata {
        pub version: [std::os::raw::c_char; 32usize],
        #[doc = " @brief Image Name"]
        pub name: [std::os::raw::c_char; 80usize],
        #[doc = " @brief Number of axis\n\n @warning 1, 2 or 3. Values above 3 not supported."]
        pub naxis: u8,
        #[doc = " @brief Image size along each axis\n\n  If naxis = 1 (1D image), size[1] and size[2] are irrelevant"]
        pub size: [u32; 3usize],
        #[doc = " @brief Number of elements in image\n\n This is computed upon image creation"]
        pub nelement: u64,
        #[doc = " @brief Data type\n\n Encoded according to data type defines.\n  -  1: uint8_t\n \t-  2: int8_t\n \t-  3: uint16_t\n \t-  4: int16_t\n \t-  5: uint32_t\n \t-  6: int32_t\n \t-  7: uint64_t\n \t-  8: int64_t\n \t-  9: IEEE 754 single-precision binary floating-point format: binary32\n  - 10: IEEE 754 double-precision binary floating-point format: binary64\n  - 11: complex_float\n  - 12: complex double\n  - 13: half precision floating-point\n"]
        pub datatype: u8,
        #[doc = "< image type"]
        pub imagetype: u64,
        pub creationtime: TimeSpec,
        pub lastaccesstime: TimeSpec,
        pub atime: TimeSpec,
        pub writetime: TimeSpec,
        #[doc = "< PID of process that created the stream (if shared = 1)"]
        pub creator_pid: std::os::raw::c_int,
        #[doc = "< PID of process owning the stream (if shared = 1)"]
        pub owner_pid: std::os::raw::c_int,
        #[doc = "< stream is in shared memory"]
        pub shared: u8,
        #[doc = "< inode nummber if shared memory"]
        pub inode: std::os::raw::c_ulong,
        #[doc = "< -1 if in CPU memory, >=0 if in GPU memory on `location` device"]
        pub location: i8,
        #[doc = "< 1 to log image (default); 0 : do not log: 2 : stop log (then goes back to 2)"]
        pub status: u8,
        #[doc = "< bitmask, encodes read/write permissions.... NOTE: enum instead of defines"]
        pub flag: u64,
        #[doc = "< set to 1 to start logging"]
        pub logflag: u8,
        #[doc = "< number of semaphores supported, specified at image creation"]
        pub sem: u16,
        #[doc = "< number of streamproctrace entries"]
        pub nb_proc_trace: u16,
        #[doc = "< counter (incremented if image is updated)"]
        pub cnt0: u64,
        #[doc = "< in 3D rolling buffer image, this is the last slice written"]
        pub cnt1: u64,
        #[doc = "< in cnt2-based syncronization, proceed until cnt0=cnt2"]
        pub cnt2: u64,
        #[doc = "< 1 if image is being written"]
        pub write: u8,
        #[doc = "< number of keywords (max: 65536)"]
        pub nb_kw: u16,
        pub cb_size: u32,
        pub cb_index: u32,
        pub cb_cycle: u64,
        pub imdatamemsize: u64,
        pub cuda_mem_handle: [std::os::raw::c_char; 64usize],
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub union Sem {
        pub __size: [std::os::raw::c_char; 32usize],
        pub __align: std::os::raw::c_long,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct SemFileData {
        pub semdata: Sem,
    }
}

/// This type has the same objects as the equivalent IMAGE struct in ISIO,
/// but importantly uses mutable references rather than raw pointers, so
/// CANNOT be mapped by bytes to the same representation in C. If this object
/// is to be used in FFI, then it will need to be mapped first to a "raw"
/// IMAGE object.
#[repr(C)]
#[derive()]
pub struct Image<'a> {
    /// local name (can be different from name in shared memory)
    pub name: [std::os::raw::c_char; 80usize],
    /// Image usage flag
    ///  - 1 if image is used
    ///  - 0 otherwise.
    ///
    /// This flag is used when an array of IMAGE type is held in memory
    /// as a way to store multiple images.
    ///
    /// When an image is freed, the corresponding memory (in array) is freed
    /// and this flag set to zero.
    ///
    /// The active images can be listed by looking for IMAGE[i].used==1 entries.
    pub used: u8,
    /// increments when image is (re)-created
    pub createcnt: i64,
    /// if shared memory, file descriptor
    pub shmfd: i32,
    /// total size in memory if shared
    pub memsize: u64,
    /// pointer to semaphore for logging  (8 bytes on 64-bit system)
    pub semlog: &'a mut Sem,
    /// pointer to image metadata
    pub md: &'a mut ImageMetadata,
    /// pointer to data array
    pub array: &'a mut [u8],

    // TODO:
    // /// array of pointers to semaphores   (each 8 bytes on 64-bit system)
    // pub semptr: &'a mut [&'a mut Sem],
    /// array of image Keywords
    pub kw: &'a mut [ImageKeyword],
    /// array of semfiles
    pub semfile: &'a mut [SemFileData],
    /// PID of process that read shared memory stream
    /// Initialized at 0. Otherwise, when process is waiting on semaphore, its PID is written in this array
    /// The array can be used to look for available semaphores
    pub sem_read_pid: &'a mut [std::os::raw::c_int],
    /// PID of processes that are posting the semaphores (JC: I guess there should usually only be one?)
    pub sem_write_pid: &'a mut [std::os::raw::c_int],
    /// semaphore control, written by writer to control semaphore behavior.
    /// See SEMAPHORE_CONTROL_XXX defines for details
    pub semctrl: &'a mut [u32],
    /// semaphore status, written by readers to report back to stream what is their current status.
    /// See SEMAPHORE_STATUS_XXX defines for details
    pub semstatus: &'a mut [u32],
    // array to keep track of stream history/depedencies
    pub streamproctrace: &'a mut [StreamProcTrace],
    // /// flag for each slice if needed (depends on imagetype)
    // pub flagarray: &'a mut [u64],
    /// For circular buffer: counter array for circular buffer, copy of cnt0 onto slice index
    pub cntarray: &'a mut [u64],
    /// For each slice index: time at which data was acquires/created.
    /// This time CAN be copied from input to output
    pub atimearray: &'a mut [TimeSpec],
    /// For each slice index: time at which data was written.
    /// This time CAN be copied from input to output
    pub writetimearray: &'a mut [TimeSpec],

    /// Circular Buffer (CB) option
    /// if CBsize>0, recent frames are memcpied in circular buffer
    /// recent frames may be accessed in small CB for logging.
    ///
    /// array of CB metadata
    pub circ_buff_md: &'a mut [CBFrameMetadata],
    /// data storage for circ buffer
    pub cb_imdata: &'a mut [u8],
    /// memory mapping
    pub mmap: Option<MmapMut>,
}
