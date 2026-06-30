use anyhow::Result;
use libc::aligned_alloc;
use rkyv::to_bytes;
use std::{io::Write, path::PathBuf, str::FromStr};

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("Invalid naxis {0}, must be 1, 2, or 3.")]
    InvalidNaxis(usize),
    #[error("Invalid size: &[usize], has length: {0}, must be 1, 2, or 3.")]
    InvalidSize(usize),
    #[error("Circular buffer must have 3 dimensions, but found naxis={0}")]
    CircBuffDimsWrong(usize),
}

const fn round_up_8(x: usize) -> usize {
    (x + 7) & !7
}

use crate::{
    DataType, ImageType, ZAxisEncodingCode,
    bindings::*
};

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

    fn fetch_err<T>() -> Result<T> {
        match std::io::Error::last_os_error().raw_os_error().unwrap() {
            x => Err(std::io::Error::from_raw_os_error(x))?,
        }
    }

    pub fn create_new_image_from_scratch(
        name: &str,
        naxis: usize,
        size: &[usize],
        datatype: DataType,
        // shared: bool,  (shared==true for now...)
        // int8_t location, (-1: CPU RAM for now)
        nb_kw: usize,
        imagetype: ImageType,
        cb_size: usize,
    ) -> Result<Self> {
        const NB_SEM: usize = IMAGE_NB_SEMAPHORE as usize;
        let nelement = size
            .iter()
            .map(|x| if *x == 0 { 1 } else { *x })
            .product::<usize>();
        let imdatamemsize = datatype.typesize() * nelement;
        let size = match size.len() {
            1 => [size[0] as u32, 0, 0],
            2 => [size[0] as u32, size[1] as u32, 0],
            3 => [size[0] as u32, size[1] as u32, size[2] as u32],
            x => return Err(Error::InvalidSize(x))?,
        };
        let naxis = match naxis {
            1..=3 => naxis,
            _ => return Err(Error::InvalidNaxis(naxis))?,
        };
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
        for semindex in 0..NB_SEM {
            let mut sem_tmp: sem_t = unsafe { std::mem::zeroed() };
            match unsafe { sem_init(&mut sem_tmp, 1, SEMAPHORE_INITVAL) } {
                e if e < 0 => Self::fetch_err()?,
                _ => (),
            };
            sem_read_pid.push(-1);
            sem_write_pid.push(-1);
            sem_ctrl.push(0);
            sem_status.push(0);
            semfile.push(SEMFILEDATA { semdata: sem_tmp });
            semptr.push(&mut semfile[semindex].semdata);
        }

        let mut semlog = sem_t { __size: [0; 32] };

        let mut stream_proc_trace: Vec<STREAM_PROC_TRACE> = vec![];
        stream_proc_trace.resize(
            IMAGE_NB_PROCTRACE as usize,
            STREAM_PROC_TRACE::new(),
        );

        let mut atimearray: Vec<timespec> = Vec::new();
        let mut writetimearray: Vec<timespec> = Vec::new();
        let mut cntarray: Vec<u64> = Vec::new();

        match imagetype {
            ImageType {
                circular_buffer: true,
                ..
            }
            | ImageType {
                axis_encoding_code: ZAxisEncodingCode::TemporalCoordinate,
                ..
            } => {
                atimearray.resize(size[2] as usize, timespec::new());
                writetimearray.resize(size[2] as usize, timespec::new());
                cntarray.resize(size[2] as usize, 0);
            }
            _ => (),
        };

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
        image_memsize += round_up_8(size_of::<timespec>() * size[2] as usize); // atimearray
        image_memsize += round_up_8(size_of::<timespec>() * size[2] as usize); // writetimearray
        image_memsize += round_up_8(size_of::<u64>() * size[2] as usize); // cntarray
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

        let mut flagarray: Vec<u64> = Vec::new(); // TODO: This isn't initialised in ISIO, but worse
        // than that, there's no memory allocated for it in the map so accessing it will probably
        // segfault.

        let mut md = IMAGE_METADATA {
            version: Self::version(),
            name: Self::_name(name),
            naxis: naxis as u8,
            size,
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

        let image = IMAGE {
            name: Self::_name(name),
            used: 1,
            createcnt: 1,
            shmfd: -1,
            memsize: image_memsize as u64,
            semlog: &mut semlog,
            md: &mut md,
            array: array_raw,
            semptr: semptr.as_mut_ptr(),
            kw: kw.as_mut_ptr(),
            semfile: semfile.as_mut_ptr(),
            semReadPID: sem_read_pid.as_mut_ptr(),
            semWritePID: sem_write_pid.as_mut_ptr(),
            semctrl: sem_ctrl.as_mut_ptr(),
            semstatus: sem_status.as_mut_ptr(),
            streamproctrace: stream_proc_trace.as_mut_ptr(),
            flagarray: flagarray.as_mut_ptr(),
            cntarray: cntarray.as_mut_ptr(),
            atimearray: atimearray.as_mut_ptr(),
            writetimearray: writetimearray.as_mut_ptr(),
            CircBuff_md: circ_buff_md.as_mut_ptr(),
            CBimdata: cb_imdata,
        };

        struct VecOwner(Vec<u8>);
        impl VecOwner {
            fn new() -> Self {
                VecOwner(vec![])
            }
            fn grow(mut self, b: &mut [u8], check: usize) -> Self {
                assert_eq!(b.len(), check);
                self.0.extend_from_slice(b); // TODO: handle an error better
                println!("  actual shmimdata length: {}", self.0.len());
                println!("len mod 8: {}", self.0.len() % 8);
                self
            }
        }

        let shmimdata = VecOwner::new()
            .grow(
                &mut to_bytes::<rkyv::rancor::Error>(&md).unwrap(),
                round_up_8(size_of::<IMAGE_METADATA>()),
            ) // TODO: handle an error better
            .grow(
                unsafe {
                    core::slice::from_raw_parts_mut(array_raw.raw.cast(), round_up_8(imdatamemsize))
                },
                imdatamemsize,
            )
            .grow(
                unsafe {
                    core::slice::from_raw_parts_mut(
                        kw.as_mut_ptr().cast(),
                        nb_kw * size_of::<IMAGE_KEYWORD>(),
                    )
                },
                round_up_8(size_of::<IMAGE_KEYWORD>() * nb_kw),
            )
            .grow(
                unsafe {
                    core::slice::from_raw_parts_mut(
                        semfile.as_mut_ptr().cast(),
                        NB_SEM * size_of::<SEMFILEDATA>(),
                    )
                },
                round_up_8(size_of::<SEMFILEDATA>() * NB_SEM),
            )
            .grow(
                &mut unsafe { semlog.__size.map(|x| x as u8) },
                round_up_8(size_of::<sem_t>()),
            )
            .grow(
                unsafe {
                    core::slice::from_raw_parts_mut(
                        sem_read_pid.as_mut_ptr().cast(),
                        size_of::<i32>() * NB_SEM,
                    )
                },
                round_up_8(size_of::<i32>() * NB_SEM),
            )
            .grow(
                unsafe {
                    core::slice::from_raw_parts_mut(
                        sem_write_pid.as_mut_ptr().cast(),
                        size_of::<i32>() * NB_SEM,
                    )
                },
                round_up_8(size_of::<i32>() * NB_SEM),
            )
            .grow(
                unsafe {
                    core::slice::from_raw_parts_mut(
                        sem_ctrl.as_mut_ptr().cast(),
                        size_of::<u32>() * NB_SEM,
                    )
                },
                round_up_8(size_of::<u32>() * NB_SEM),
            )
            .grow(
                unsafe {
                    core::slice::from_raw_parts_mut(
                        sem_write_pid.as_mut_ptr().cast(),
                        size_of::<u32>() * NB_SEM,
                    )
                },
                round_up_8(size_of::<u32>() * NB_SEM),
            )
            .grow(
                unsafe {
                    core::slice::from_raw_parts_mut(
                        stream_proc_trace.as_mut_ptr().cast(),
                        size_of::<STREAM_PROC_TRACE>() * nbproctrace,
                    )
                },
                round_up_8(size_of::<STREAM_PROC_TRACE>() * nbproctrace),
            )
            .grow(
                unsafe {
                    core::slice::from_raw_parts_mut(
                        atimearray.as_mut_ptr().cast(),
                        size_of::<timespec>() * size[2] as usize,
                    )
                },
                round_up_8(size_of::<timespec>() * size[2] as usize),
            )
            .grow(
                unsafe {
                    core::slice::from_raw_parts_mut(
                        writetimearray.as_mut_ptr().cast(),
                        size_of::<timespec>() * size[2] as usize,
                    )
                },
                round_up_8(size_of::<timespec>() * size[2] as usize),
            )
            .grow(
                unsafe {
                    core::slice::from_raw_parts_mut(
                        cntarray.as_mut_ptr().cast(),
                        size_of::<u64>() * size[2] as usize,
                    )
                },
                round_up_8(size_of::<u64>() * size[2] as usize),
            )
            .grow(
                unsafe {
                    core::slice::from_raw_parts_mut(
                        circ_buff_md.as_mut_ptr().cast(),
                        size_of::<CBFRAMEMD>() * cb_size,
                    )
                },
                round_up_8(size_of::<CBFRAMEMD>() * cb_size),
            )
            .grow(
                unsafe {
                    core::slice::from_raw_parts_mut(cb_imdata.cast(), imdatamemsize * cb_size)
                },
                round_up_8(imdatamemsize * cb_size),
            );

        println!("expected shmimdata length: {}", image_memsize);

        use memmap2::MmapMut;

        let file = std::fs::File::create_new(Self::sm_pname(name)?)?;
        file.set_len(image_memsize as u64)?;
        let mut mmap = unsafe { MmapMut::map_mut(&file)? };
        (&mut mmap[..]).write_all(&shmimdata.0)?;
        Ok(image)
    }
}
