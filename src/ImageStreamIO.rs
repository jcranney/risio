use anyhow::Result;
use std::{
    ffi::{CStr, c_void},
    mem::MaybeUninit,
    ptr::addr_of,
    slice::from_raw_parts,
    str::from_boxed_utf8_unchecked,
};

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
    x + 7 & !7
}

use crate::{
    DataType, ImageType, ZAxisEncodingCode,
    bindings::{
        self, CBFRAMEMD, IMAGE, IMAGE__bindgen_ty_1, IMAGE_KEYWORD, IMAGE_KEYWORD__bindgen_ty_1,
        IMAGE_METADATA, SEMFILEDATA, STREAM_PROC_TRACE, pid_t, sem_t, timespec,
    },
};

impl IMAGE {
    // I'm going to start with an implementation that only considers CPU SHM,
    // i.e., ignoring non-shared cases, and GPU cases. I'll probably need to
    // add the non-shared case later, but I don't see myself ever understanding
    // GPU interfacing enough to get that working.
    unsafe fn create_im(
        name: &CStr,
        naxis: usize,
        size: &[usize],
        datatype: DataType,
        // shared: bool,  (shared==true for now...)
        nb_sem: usize,
        nb_kw: usize,
        imagetype: ImageType,
        cb_size: usize,
    ) -> Self {
        let mut image = MaybeUninit::<IMAGE>::uninit();
        let mut md = unsafe { std::mem::zeroed::<bindings::IMAGE_METADATA>() };

        todo!()
    }

    fn _name(name: &str) -> [i8; bindings::STRINGMAXLEN_IMAGE_NAME as usize] {
        let mut out = [0; bindings::STRINGMAXLEN_IMAGE_NAME as usize];
        for (out_i, in_i) in out.iter_mut().zip(name.as_bytes()) {
            *out_i = *in_i as i8;
        }
        // must be null terminated, so let's force that here to be extra safe
        *out.last_mut().unwrap() = 0;

        out
    }

    fn sm_fname(name: &str) -> [i8; 200] {
        let mut s: [i8; 200] = [0; 200];
        s.iter_mut()
            .zip(Self::_name(name))
            .for_each(|(a, b)| *a = b as i8);
        s
    }

    fn version() -> [i8; 32] {
        let mut version = [0; 32];
        version
            .iter_mut()
            .zip(bindings::IMAGESTRUCT_VERSION)
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

    fn create_new_image_from_scratch(
        name: &str,
        naxis: usize,
        size: &[usize],
        datatype: DataType,
        // shared: bool,  (shared==true for now...)
        // int8_t location, (-1: CPU RAM for now)
        nb_sem: usize,
        nb_kw: usize,
        imagetype: ImageType,
        cb_size: usize,
    ) -> Result<Self> {
        let nelement = (size
            .iter()
            .map(|x| if *x == 0 { 1 } else { *x })
            .product::<usize>());
        let imdatamemsize = datatype.typesize() * nelement;
        let size = match size.len() {
            1 => [size[0] as u32, 0, 0],
            2 => [size[0] as u32, size[1] as u32, 0],
            3 => [size[0] as u32, size[1] as u32, size[2] as u32],
            x => return Err(Error::InvalidSize(x))?,
        };
        let naxis = match naxis {
            1 | 2 | 3 => naxis,
            _ => return Err(Error::InvalidNaxis(naxis))?,
        };
        if imagetype.circular_buffer && (naxis != 3) {
            return Err(Error::CircBuffDimsWrong(naxis))?;
        }

        let mut array_init: Vec<u8> = vec![];
        array_init.resize(imdatamemsize, 0);
        let array_raw = IMAGE__bindgen_ty_1 {
            UI8: array_init.clone().as_mut_ptr(),
        };

        let nbproctrace = bindings::IMAGE_NB_PROCTRACE as usize;

        let mut kw: Vec<IMAGE_KEYWORD> = (0..nb_kw).map(|_| IMAGE_KEYWORD::new()).collect();

        // - Assign pointers; initialize the semphores and their data
        let mut semptr: Vec<*mut sem_t> = vec![];
        let mut sem_read_pid: Vec<i32> = vec![];
        let mut sem_write_pid: Vec<i32> = vec![];
        let mut sem_ctrl: Vec<u32> = vec![];
        let mut sem_status: Vec<u32> = vec![];
        let mut semfile: Vec<SEMFILEDATA> = vec![];
        for semindex in 0..nb_sem {
            let sem_tmp = std::ptr::null_mut::<sem_t>();
            unsafe {
                libc::sem_init(
                    addr_of!(sem_tmp) as *mut libc::sem_t,
                    1,
                    bindings::SEMAPHORE_INITVAL,
                )
            };
            sem_read_pid.push(-1);
            sem_write_pid.push(-1);
            sem_ctrl.push(0);
            sem_status.push(0);
            semfile.push(SEMFILEDATA {
                semdata: unsafe { *sem_tmp },
            });
            semptr.push(&mut semfile[semindex].semdata);
        }

        let mut semlog = sem_t { __size: [0; 32] };

        let mut stream_proc_trace: Vec<STREAM_PROC_TRACE> = vec![];
        stream_proc_trace.resize(
            bindings::IMAGE_NB_PROCTRACE as usize,
            STREAM_PROC_TRACE::new(),
        );

        let mut atimearray: Vec<bindings::timespec> = Vec::new();
        let mut writetimearray: Vec<bindings::timespec> = Vec::new();
        let mut cntarray: Vec<u64> = Vec::new();

        match imagetype {
            ImageType {
                circular_buffer: true,
                axis_encoding_code: ZAxisEncodingCode::TemporalCoordinate,
                ..
            } => {
                atimearray.resize(size[2] as usize, bindings::timespec::new());
                writetimearray.resize(size[2] as usize, bindings::timespec::new());
                cntarray.resize(size[2] as usize, 0);
            }
            _ => (),
        };

        let mut circ_buff_md = Vec::new();
        circ_buff_md.resize(cb_size, CBFRAMEMD::new());

        // let mut cb_imdata: Vec<IMAGE__bindgen_ty_1> = Vec::new();
        // cb_imdata.resize(
        //     cb_size,
        //     IMAGE__bindgen_ty_1 {
        //         UI8: array_init.clone().as_mut_ptr(),
        //     },
        // );
        let cb_imdata = unsafe { libc::malloc(imdatamemsize * cb_size) };

        let mut image_memsize: usize = round_up_8(size_of::<IMAGE_METADATA>());
        image_memsize += round_up_8(imdatamemsize);
        image_memsize += round_up_8(size_of::<IMAGE_KEYWORD>() * nb_kw);
        image_memsize += round_up_8(size_of::<SEMFILEDATA>() * nb_sem);
        image_memsize += round_up_8(size_of::<sem_t>()); // semlog
        image_memsize += round_up_8(size_of::<pid_t>() * nb_sem); // semReadPID
        image_memsize += round_up_8(size_of::<pid_t>() * nb_sem); // semWritePID
        image_memsize += round_up_8(size_of::<u32>() * nb_sem); // semctrl
        image_memsize += round_up_8(size_of::<u32>() * nb_sem); // semstatus
        image_memsize += round_up_8(size_of::<STREAM_PROC_TRACE>() * nbproctrace);
        image_memsize += round_up_8(size_of::<timespec>() * size[2] as usize); // atimearray
        image_memsize += round_up_8(size_of::<timespec>() * size[2] as usize); // writetimearray
        image_memsize += round_up_8(size_of::<u64>() * size[2] as usize); // cntarray
        image_memsize += round_up_8(size_of::<CBFRAMEMD>() * cb_size);
        image_memsize += round_up_8(imdatamemsize * cb_size);

        unsafe { libc::umask(0) }; // TODO: I have no idea why this umask is needed

        let fd: i32 = match unsafe {
            libc::shm_open(
                Self::sm_fname(name).as_ptr(),
                libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_TRUNC,
                0o0666,
            )
        } {
            e if e < 0 => Self::fetch_err()?,
            fd => fd,
        };

        // - Seek to the end of the currently empty shmim file, ...
        match unsafe { libc::lseek(fd, image_memsize as i64 - 1, libc::SEEK_SET) } {
            e if e < 0 => Self::fetch_err()?,
            offset if offset as usize == (image_memsize - 1) => (),
            _ => unreachable!(),
        };

        // - ... then write a null at that sought position
        match unsafe { libc::write(fd, std::ptr::null(), 1) } {
            e if e < 0 => Self::fetch_err().inspect_err(|_| {
                unsafe { libc::close(fd) };
            })?,
            1 => (),
            _ => unreachable!(),
        };

        let mut map = match unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                image_memsize,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        } {
            libc::MAP_FAILED => Self::fetch_err()?,
            map => map,
        };

        let mut file_stat: libc::stat = unsafe { std::mem::zeroed() };
        match unsafe { libc::fstat(fd, &mut file_stat) } {
            e if e < 0 => {
                unsafe { libc::close(fd) };
                Self::fetch_err()?;
            }
            0 => (),
            _ => unreachable!(),
        };

        let mut last_access_time = bindings::timespec::new();
        unsafe { bindings::clock_gettime(bindings::CLOCK_TAI as i32, &mut last_access_time) };
        let mut creation_time = bindings::timespec::new();
        unsafe { bindings::clock_gettime(bindings::CLOCK_TAI as i32, &mut creation_time) };

        let mut flagarray: Vec<u64> = Vec::new(); // TODO: This isn't initialised in ISIO, but worse
        // than that, there's no memory allocated for it in the map so accessing it will probably
        // segfault.

        let mut md = bindings::IMAGE_METADATA {
            version: Self::version(),
            name: Self::_name(name),
            naxis: naxis as u8,
            size,
            nelement: nelement as u64,
            datatype: datatype as u8,
            imagetype: imagetype.into(),
            creationtime: creation_time,
            lastaccesstime: last_access_time,
            atime: bindings::timespec::new(),
            writetime: bindings::timespec::new(),
            creatorPID: unsafe { libc::getpid() },
            ownerPID: 0,
            shared: 1,
            inode: file_stat.st_ino,
            location: -1,
            status: 0,  // TODO: find initialisastion in C library.
            flag: 0, // TODO: find initialisastion in C library.
            logflag: 0, // TODO: find initialisastion in C library.
            sem: nb_sem as u16,
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
            cudaMemHandle: [0; 64],  // TODO: find initialisastion in C library.
        };

        let image = IMAGE {
            name: Self::_name(name),
            used: 1,
            createcnt: 1,
            shmfd: fd,
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

        let x = &raw mut md;
        // bytemuck::cast_slice_mut::<IMAGE_METADATA, u8>(md);

        // map.copy_from(src, count);

        Ok(image)
    }
}
