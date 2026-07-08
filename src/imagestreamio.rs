use crate::bindings::*;
use crate::error::Error;
use crate::datatype::*;
use memmap2::MmapMut;
use std::ffi::c_void;
use std::slice::from_raw_parts;
use std::{path::PathBuf, str::FromStr};

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

    fn sm_pname(name: &str) -> Result<PathBuf, Error> {
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

    fn fetch_io_err<T>() -> Result<T, Error> {
        let x = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        Err(std::io::Error::from_raw_os_error(x))?
    }

    // fn to_mmap_mut(self) -> Result<MmapMut> {
    //     todo!()
    // }

    fn from_mmap_mut(mmap: &mut MmapMut) -> Result<Self, Error> {
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
            semlog,
            md,
            array,
            semptr,
            kw,
            semfile,
            semReadPID: sem_read_pid,
            semWritePID: sem_write_pid,
            semctrl: sem_ctrl,
            semstatus: sem_status,
            streamproctrace: stream_proc_trace,
            flagarray: [0; 10].as_mut_ptr(),
            cntarray,
            atimearray,
            writetimearray,
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
    ) -> Result<(Self, MmapMut), Error> {
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
            match unsafe { libc::sem_init(&mut semfile_tmp.semdata, 1, SEMAPHORE_INITVAL) } {
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
        match unsafe { libc::sem_init(semlog, 1, SEMAPHORE_INITVAL) } {
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
        atimearray.resize(len_timedim, timespec{ tv_sec: 0, tv_nsec: 0 });
        writetimearray.resize(len_timedim, timespec{ tv_sec: 0, tv_nsec: 0 });
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

        let mut last_access_time = libc::timespec{ tv_sec: 0, tv_nsec: 0 };
        unsafe { libc::clock_gettime(CLOCK_TAI as i32, &mut last_access_time) };
        let mut creation_time = libc::timespec{ tv_sec: 0, tv_nsec: 0 };
        unsafe { libc::clock_gettime(CLOCK_TAI as i32, &mut creation_time) };

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
            atime: timespec{ tv_sec: 0, tv_nsec: 0 },
            writetime: timespec{ tv_sec: 0, tv_nsec: 0 },
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
            fn new(file: std::fs::File) -> Result<Self, Error> {
                Ok(Self {
                    map: unsafe { MmapMut::map_mut(&file)? },
                    idx: 0,
                })
            }
            fn get_next_mut_ptr(&mut self, len: usize) -> Result<&mut [u8], Error> {
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
                core::slice::from_raw_parts(
                    kw.as_ptr().cast(),
                    nb_kw * size_of::<IMAGE_KEYWORD>(),
                )
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
                core::slice::from_raw_parts(
                    sem_read_pid.as_ptr().cast(),
                    size_of::<i32>() * NB_SEM,
                )
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
                core::slice::from_raw_parts(
                    sem_ctrl.as_ptr().cast(),
                    size_of::<u32>() * NB_SEM,
                )
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
                core::slice::from_raw_parts(
                    cb_imdata.as_ptr().cast(),
                    imdatamemsize * cb_size,
                )
            });
        assert_eq!(
            image_memsize, mmap.idx,
            "unexpected data size, something is wrong."
        );
        mmap.map.flush()?;

        let image = Self::from_mmap_mut(&mut mmap.map)?;
        Ok((image, mmap.map))
    }

    pub fn open_image(name: &str) -> Result<(Self, MmapMut), Error> {
        let file = std::fs::File::options()
            .read(true)
            .write(true)
            .open(Self::sm_pname(name)?)?;

        let mut mmap = unsafe { MmapMut::map_mut(&file) }?;

        let image = Self::from_mmap_mut(&mut mmap)?;
        Ok((image, mmap))
    }
}
