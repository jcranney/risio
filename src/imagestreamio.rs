use crate::TimeSpec;
use crate::bindings::*;
use crate::datatype::*;
use crate::error::Error;
use crate::imagestreamio::byte_structs::*;
use byte_structs::Byteable;
use memmap2::MmapMut;
use std::fs::OpenOptions;
use std::slice::from_raw_parts_mut;
use std::{path::PathBuf, str::FromStr};

const fn round_up_8(x: usize) -> usize {
    (x + 7) & !7
}

#[derive(Debug, Clone)]
pub struct ShmImOpenOptions {
    /// An image will always be at least read
    read: bool,
    /// An image will often also be write
    write: bool,
    /// An image will never be append
    append: bool,
    /// An image should usually not be truncated, since this destroys
    /// the previously contained memory on an image.
    truncate: bool,
    /// We opt for a low number of strict opening modes, to avoid ambiguity
    /// in implementation, at the cost of a slightly more verbose opening
    /// scheme. `create==true` implies that an image should be opened if it
    /// exists, or created if it does not. This functionality will exist in this
    /// library, but not via the "create" flag when the image is opened. Instead
    /// the library call will try to create the image exclusively, and then
    /// upon failure try to open the image.
    create: bool,
    /// A new image will always be created with `create_new==true`.
    create_new: bool,
}

impl Default for ShmImOpenOptions {
    /// The default opening options match a "read/write" usage, but will not
    /// create an image.
    fn default() -> Self {
        ShmImOpenOptions {
            read: true,
            write: true,
            append: false,
            truncate: false,
            create: false,
            create_new: false,
        }
    }
}

/// An image can only be opened with `ShmImOpenOptions::default()`, which opens
/// the image in read/write mode if it already exists.
///
/// To create an image that doesn't exist, use
/// `ShmImOpenOptions::default().create_new()`
impl ShmImOpenOptions {
    /// This will allow the creation of a new image, failing if it already
    /// exists. If the image is created, it's length is initially zero.
    pub fn create_new(&mut self) -> &mut Self {
        self.create_new = true;
        self.truncate = true;
        self
    }
}

impl From<&mut ShmImOpenOptions> for OpenOptions {
    fn from(shmim_options: &mut ShmImOpenOptions) -> Self {
        let ShmImOpenOptions {
            read,
            write,
            append,
            truncate,
            create,
            create_new,
        } = *shmim_options;
        let mut std_options = Self::new();
        std_options
            .read(read)
            .write(write)
            .append(append)
            .truncate(truncate)
            .create(create)
            .create_new(create_new);
        std_options
    }
}

impl<'a, T> Image<'a, T> {
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
}

/// The creation process of the image is intended to be strict, such that only
/// valid images can ever be created from this interface. Then, additionally,
/// there should be a sanitiser (independent of memory location) that checks if
/// an Image instance is valid in all of it's datatypes and consistent with any
/// mutually dependent values (e.g., check that the specified number of elements is equal
/// to the product of the non-zero dimensions, etc.).
///
/// Since an image can be created outside of this crate, the sanitiser should run
/// on all externally crated images, and if the cost is low, also on internally
/// created images. Since an image can be updated at any time through any external
/// process, the "safest" way to operate would be to run the sanitiser before doing
/// any local operation, but this is likely to introduce too much latency in the
/// hard real-time loops that ISIO targets. A compromise could be to run the
/// sanitiser after each "write" method, once the semaphore has been posted, since
/// this will likely not be blocking any time-crititcal tasks. In any case, there
/// should be an "*_unchecked" method for those tasks to allow for the user to
/// sacrifice safety for speed.
impl<'a> Image<'a, Vec<u8>> {
    /// Create and initialise a new image locally with specified properties.
    pub fn new_local(
        name: &str,
        shape: &[usize],
        datatype: DataType,
        nb_kw: usize,
        imagetype: ImageType,
        cb_size: usize,
    ) -> Result<Self, Error> {
        let naxis: usize = shape.len();

        // for the rust interface, we asserting that all axes are non-zero in
        // dimsion.
        if shape.iter().any(|s| s == &0) {
            return Err(Error::InvalidShapeArray {
                shape: shape.to_vec(),
            });
        }

        // create a new shape3 with constant size 3 (like ISIO format)
        let shape3d = match shape.len() {
            0 => [0, 0, 0],
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
        let nelement = if shape.is_empty() {
            0
        } else {
            shape.iter().product::<usize>()
        };

        // data memory size is the size of the type, by the number of elements
        let imdatamemsize = datatype.typesize() * nelement;

        if imagetype.circular_buffer && (naxis != 3) {
            return Err(Error::CircBuffDimsWrong(naxis))?;
        }

        let mut last_access_time: TimeSpec = TimeSpec::new();
        last_access_time.get_time();
        let mut creation_time = TimeSpec::new();
        creation_time.get_time();

        let md = ImageMetadata {
            version: Self::version(),
            name: Self::_name(name),
            naxis: naxis as u8,
            size: shape3d,
            nelement: nelement as u64,
            datatype: datatype.into(),
            imagetype: imagetype.into(),
            creationtime: creation_time,
            lastaccesstime: last_access_time,
            atime: TimeSpec::new(),
            writetime: TimeSpec::new(),
            creator_pid: std::process::id() as i32,
            owner_pid: 0,
            shared: 1,
            inode: 0, // file_stat.st_ino, TODO: learn why this is needed and set it properly
            location: -1,
            status: 0,  // TODO: find initialisastion in C library.
            flag: 0,    // TODO: find initialisastion in C library.
            logflag: 0, // TODO: find initialisastion in C library.
            sem: IMAGE_NB_SEMAPHORE as u16,
            nb_proc_trace: IMAGE_NB_PROCTRACE as u16,
            cnt0: 0,
            cnt1: 0,
            cnt2: 0, // TODO: find initialisastion in C library.
            write: 0,
            nb_kw: nb_kw as u16,
            cb_size: cb_size as u32,
            cb_index: 0,
            cb_cycle: 0,
            imdatamemsize: imdatamemsize as u64,
            cuda_mem_handle: [0; 64], // TODO: find initialisastion in C library.
        };
        md.try_into()
    }

    /// Take a locally defined Image and map it to a file - consuming the
    /// original Image in the process. This operation moves the underlying data,
    /// but does not alter it in the process.
    pub fn to_mmapped(self, filename: &str) -> Result<Image<'a, MmapMut>, Error> {
        let options: OpenOptions = ShmImOpenOptions::default().create_new().into();
        let file = options.open(Self::sm_pname(filename)?)?;
        file.set_len(self.data_handle.len() as u64)?;
        let mut mmap = unsafe { MmapMut::map_mut(&file) }?;
        mmap.copy_from_slice(&self.data_handle);
        Image::try_from(mmap)
    }
}

impl<'a> Image<'a, MmapMut> {
    pub unsafe fn new_shm(
        filename: &str,
        name: &str,
        shape: &[usize],
        datatype: DataType,
        nb_kw: usize,
        imagetype: ImageType,
        cb_size: usize,
    ) -> Result<Self, Error> {
        let local_im = Image::new_local(name, shape, datatype, nb_kw, imagetype, cb_size)?;
        local_im.to_mmapped(filename)
    }

    pub unsafe fn open_image(filename: &str) -> Result<Self, Error> {
        let options: OpenOptions = (&mut ShmImOpenOptions::default()).into();
        let file = options.open(Self::sm_pname(filename)?)?;

        let mmap = unsafe { MmapMut::map_mut(&file) }?;

        let image = Self::try_from(mmap)?;
        // image.mmap = Some(mmap);
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

    use std::slice::from_raw_parts;

    use crate::bindings::{CLOCK_TAI, SEMAPHORE_INITVAL, STRINGMAXLEN_IMAGE_NAME};

    pub trait Byteable {
        fn to_bytes(self) -> Vec<u8>;
    }

    impl<T: Byteable> Byteable for Vec<T> {
        fn to_bytes(self) -> Vec<u8> {
            self.into_iter().flat_map(|x| x.to_bytes()).collect()
        }
    }

    /// The ImageKeyword structure contains a name, value, count and comment,
    /// and a c_char valued discriminant with the legal values of:
    ///  - b'N', unused,
    ///  - b'L', long,
    ///  - b'D', double,
    ///  - b'S', 16-char string.
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct ImageKeyword {
        pub name: [u8; 16],
        /// N: unused, L: long, D: double, S: 16-char string
        pub keyword_type: u8,
        pub value: KeywordValue,
        pub cnt: u64,
        pub comment: [u8; 80],
    }

    impl Default for ImageKeyword {
        fn default() -> Self {
            Self::new()
        }
    }

    impl ImageKeyword {
        pub fn new() -> Self {
            Self {
                name: [0; 16],
                keyword_type: b'N',
                value: KeywordValue { numl: 0 },
                cnt: 0,
                comment: [0; 80usize],
            }
        }
    }

    impl Byteable for ImageKeyword {
        fn to_bytes(mut self) -> Vec<u8> {
            unsafe { from_raw_parts((&mut self as *mut Self).cast(), size_of::<Self>()) }.to_vec()
        }
    }

    /// Keyword value - in order to reference directly from memory we keep a
    /// union representation, despite that being unsafe and annoying.
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub union KeywordValue {
        numl: i64,
        numf: f64,
        valstr: [u8; 16],
    }

    /// Semaphore value, wrapper around libc semaphore, allowing reliable conversion
    /// between value and bytes-representation
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Sem {
        sem: libc::sem_t,
    }

    impl Byteable for Sem {
        fn to_bytes(mut self) -> Vec<u8> {
            unsafe { from_raw_parts((&mut self as *mut Self).cast(), size_of::<Self>()) }.to_vec()
        }
    }

    impl Default for Sem {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Sem {
        // Create and initialise new libc::sem_t type, wrapped in Sem type. If
        // the underlying C-call fails (which it never should), panic.
        pub fn new() -> Self {
            let mut sem: libc::sem_t = unsafe { std::mem::zeroed() };
            if unsafe { libc::sem_init(&mut sem, 1, SEMAPHORE_INITVAL) } < 0 {
                panic!("somehow unable to initialise new semaphore!");
            }
            Self { sem }
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct SemPid {
        pid: i32,
    }

    impl Byteable for SemPid {
        fn to_bytes(self) -> Vec<u8> {
            let SemPid { pid } = self;
            pid.to_ne_bytes().to_vec()
        }
    }

    impl Default for SemPid {
        fn default() -> Self {
            Self::new()
        }
    }

    impl SemPid {
        // initialise the PID value to -1, as per ISIO convention
        pub fn new() -> Self {
            Self { pid: -1 }
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct SemCtrl {
        ctrl: u32,
    }

    impl Byteable for SemCtrl {
        fn to_bytes(self) -> Vec<u8> {
            let SemCtrl { ctrl } = self;
            ctrl.to_ne_bytes().to_vec()
        }
    }

    impl Default for SemCtrl {
        fn default() -> Self {
            Self::new()
        }
    }

    impl SemCtrl {
        pub fn new() -> Self {
            Self { ctrl: 0 }
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct SemStatus {
        status: u32,
    }

    impl Byteable for SemStatus {
        fn to_bytes(self) -> Vec<u8> {
            let SemStatus { status } = self;
            status.to_ne_bytes().to_vec()
        }
    }

    impl Default for SemStatus {
        fn default() -> Self {
            Self::new()
        }
    }

    impl SemStatus {
        pub fn new() -> Self {
            Self { status: 0 }
        }
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

    impl Byteable for StreamProcTrace {
        fn to_bytes(mut self) -> Vec<u8> {
            unsafe { from_raw_parts((&mut self as *mut Self).cast(), size_of::<Self>()) }.to_vec()
        }
    }

    impl Default for StreamProcTrace {
        fn default() -> Self {
            Self::new()
        }
    }

    impl StreamProcTrace {
        pub fn new() -> Self {
            Self {
                triggermode: 0,
                procwrite_pid: 0,
                trigger_inode: 0,
                ts_procstart: TimeSpec::new(),
                ts_streamupdate: TimeSpec::new(),
                trigsemindex: -1,
                triggerstatus: 0,
                cnt0: 0,
            }
        }
    }

    /// Circular Buffer Frame Metadata
    #[repr(C)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct CBFrameMetadata {
        cnt0: Cnt,
        cnt1: Cnt,
        atime: TimeSpec,
        writetime: TimeSpec,
    }

    impl Default for CBFrameMetadata {
        fn default() -> Self {
            Self::new()
        }
    }

    impl CBFrameMetadata {
        pub fn new() -> Self {
            Self {
                cnt0: Cnt::new(),
                cnt1: Cnt::new(),
                atime: TimeSpec::new(),
                writetime: TimeSpec::new(),
            }
        }
    }

    impl Byteable for CBFrameMetadata {
        fn to_bytes(mut self) -> Vec<u8> {
            unsafe { from_raw_parts((&mut self as *mut Self).cast(), size_of::<Self>()) }.to_vec()
        }
    }

    /// TimeSpec, used for any "instant" style timestamp.
    #[repr(C)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct TimeSpec {
        pub tv_sec: u64,
        pub tv_nsec: u64,
    }

    impl Default for TimeSpec {
        fn default() -> Self {
            Self::new()
        }
    }

    impl TimeSpec {
        pub fn new() -> Self {
            Self {
                tv_sec: 0,
                tv_nsec: 0,
            }
        }

        pub fn get_time(&mut self) -> &mut Self {
            unsafe { libc::clock_gettime(CLOCK_TAI as i32, (self as *mut TimeSpec).cast()) };
            self
        }
    }

    impl Byteable for TimeSpec {
        fn to_bytes(mut self) -> Vec<u8> {
            unsafe { from_raw_parts((&mut self as *mut Self).cast(), size_of::<Self>()) }.to_vec()
        }
    }

    /// Counter type, just a u64 wrapper
    #[repr(C)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct Cnt {
        cnt: u64,
    }

    impl Default for Cnt {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Cnt {
        pub fn new() -> Self {
            Self { cnt: 0 }
        }
    }

    impl Byteable for Cnt {
        fn to_bytes(self) -> Vec<u8> {
            self.cnt.to_ne_bytes().to_vec()
        }
    }

    /// Image metadata
    #[repr(C)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct ImageMetadata {
        pub version: [std::os::raw::c_char; 32usize],
        #[doc = " @brief Image Name"]
        pub name: [std::os::raw::c_char; STRINGMAXLEN_IMAGE_NAME as usize],
        #[doc = " @brief Number of axis\n\n @warning 1, 2 or 3. Values above 3 not supported."]
        pub naxis: u8,
        #[doc = " @brief Image size along each axis\n\n  If naxis = 1 (1D image), `size[1]` and `size[2]` are irrelevant"]
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

    impl ImageMetadata {
        pub fn to_bytes(mut self) -> Vec<u8> {
            unsafe {
                from_raw_parts(
                    (&mut self as *mut ImageMetadata).cast(),
                    size_of::<ImageMetadata>(),
                )
            }
            .to_vec()
        }
    }
}

/// This type has the same objects as the equivalent IMAGE struct in ISIO,
/// but importantly uses mutable references rather than raw pointers, so
/// CANNOT be mapped by bytes to the same representation in C. If this object
/// is to be used in FFI, then it will need to be mapped first to a "raw"
/// IMAGE object.
#[repr(C)]
#[derive()]
pub struct Image<'a, T> {
    /// local name (can be different from name in shared memory)
    pub name: [std::os::raw::c_char; STRINGMAXLEN_IMAGE_NAME as usize],
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
    /// The active images can be listed by looking for `IMAGE[i].used==1` entries.
    pub used: u8,
    /// increments when image is (re)-created
    pub create_cnt: i64,
    // /// if shared memory, file descriptor
    // pub shm_fd: i32,
    // /// total size in memory if shared
    // pub mem_size: u64,
    /// pointer to semaphore for logging  (8 bytes on 64-bit system)
    pub sem_log: &'a mut libc::sem_t,
    /// pointer to image metadata
    pub md: &'a mut ImageMetadata,
    /// pointer to data array
    pub array: &'a mut [u8],

    /// array of pointers to semaphores   (each 8 bytes on 64-bit system)
    // pub semptr: &'a mut [&'a mut libc::sem_t],
    /// array of image Keywords
    pub kw: &'a mut [ImageKeyword],
    /// array of semfiles
    pub sem_file: &'a mut [libc::sem_t],
    /// PID of process that read shared memory stream
    /// Initialized at 0. Otherwise, when process is waiting on semaphore, its PID is written in this array
    /// The array can be used to look for available semaphores
    pub sem_read_pid: &'a mut [std::os::raw::c_int],
    /// PID of processes that are posting the semaphores (JC: I guess there should usually only be one?)
    pub sem_write_pid: &'a mut [std::os::raw::c_int],
    /// semaphore control, written by writer to control semaphore behavior.
    /// See SEMAPHORE_CONTROL_XXX defines for details
    pub sem_ctrl: &'a mut [u32],
    /// semaphore status, written by readers to report back to stream what is their current status.
    /// See SEMAPHORE_STATUS_XXX defines for details
    pub sem_status: &'a mut [u32],
    // array to keep track of stream history/depedencies
    pub stream_proc_trace: &'a mut [StreamProcTrace],
    // /// flag for each slice if needed (depends on imagetype)
    // pub flagarray: &'a mut [u64],
    /// For circular buffer: counter array for circular buffer, copy of cnt0 onto slice index
    pub cnt_array: &'a mut [u64],
    /// For each slice index: time at which data was acquires/created.
    /// This time CAN be copied from input to output
    pub a_time_array: &'a mut [TimeSpec],
    /// For each slice index: time at which data was written.
    /// This time CAN be copied from input to output
    pub write_time_array: &'a mut [TimeSpec],

    /// Circular Buffer (CB) option
    /// if CBsize>0, recent frames are memcpied in circular buffer
    /// recent frames may be accessed in small CB for logging.
    ///
    /// array of CB metadata
    pub circ_buff_md: &'a mut [CBFrameMetadata],
    /// data storage for circ buffer
    pub cb_im_data: &'a mut [u8],
    /// memory mapping
    pub data_handle: T,
}

/// This enum aims to capture the "owned" data, being a vec of u8's if the
/// image only exists locally, or a MmapMut if the image is in shared memory
pub enum Sharing {
    Local(Vec<u8>),
    Mmap(MmapMut),
}

// Quite a lot of duplication in these
impl TryFrom<MmapMut> for Image<'_, MmapMut> {
    type Error = Error;

    fn try_from(mut mmap: MmapMut) -> Result<Self, Self::Error> {
        // so now we want to populate the data in a new IMAGE from mmap.
        // I guess either the mmap data is contiguous, or the IMAGE data is
        // contiguous, not both. So perhaps the IMAGE data is all simply cloned
        // from the SHM, including pointers etc.s

        // first up is image metadata.
        let (chunk, leftover) = mmap.split_at_mut(round_up_8(size_of::<ImageMetadata>()));
        let md: &mut ImageMetadata = unsafe {
            chunk
                .as_mut_ptr()
                .cast::<ImageMetadata>()
                .as_mut_unchecked()
        };

        let md_tmp = md.clone();
        DataType::try_from(md_tmp.datatype)?;

        // image array:
        let (chunk, leftover) = leftover.split_at_mut(round_up_8(md_tmp.imdatamemsize as usize));
        let array: &mut [u8] = unsafe {
            from_raw_parts_mut(
                chunk.as_mut_ptr(),
                round_up_8(md_tmp.imdatamemsize as usize),
            )
        };
        let (chunk, leftover) = leftover.split_at_mut(round_up_8(
            size_of::<ImageKeyword>() * md_tmp.nb_kw as usize,
        ));
        let kw: &mut [ImageKeyword] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.nb_kw as usize) };

        let (chunk, leftover) =
            leftover.split_at_mut(round_up_8(size_of::<libc::sem_t>() * md_tmp.sem as usize));
        let sem_file: &mut [libc::sem_t] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.sem as usize) };

        let (chunk, leftover) = leftover.split_at_mut(round_up_8(size_of::<libc::sem_t>()));
        let sem_log: &mut libc::sem_t =
            { unsafe { chunk.as_mut_ptr().cast::<libc::sem_t>().as_mut_unchecked() } };

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
        let a_time_array: &mut [TimeSpec] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.size[2] as usize) };

        let (chunk, leftover) =
            leftover.split_at_mut(round_up_8(size_of::<TimeSpec>() * md_tmp.size[2] as usize));
        let write_time_array: &mut [TimeSpec] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.size[2] as usize) };

        let (chunk, leftover) =
            leftover.split_at_mut(round_up_8(size_of::<u64>() * md_tmp.size[2] as usize));
        let cnt_array: &mut [u64] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.size[2] as usize) };

        let (chunk, leftover) = leftover.split_at_mut(round_up_8(
            size_of::<CBFrameMetadata>() * md_tmp.cb_size as usize,
        ));
        let circ_buff_md: &mut [CBFrameMetadata] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.cb_size as usize) };

        let (chunk, leftover) = leftover.split_at_mut(round_up_8(
            md_tmp.imdatamemsize as usize * md_tmp.cb_size as usize,
        ));
        let cb_im_data: &mut [u8] = unsafe {
            from_raw_parts_mut(
                chunk.as_mut_ptr(),
                round_up_8(md_tmp.imdatamemsize as usize * md_tmp.cb_size as usize),
            )
        };

        // should be nothing left in mmap:
        assert_eq!(leftover.len(), 0);

        let image = Self {
            name: md_tmp.name,
            used: 1,
            create_cnt: 1,
            sem_log,
            md,
            array,
            // // semptr,
            kw,
            sem_file,
            sem_read_pid,
            sem_write_pid,
            sem_ctrl,
            sem_status,
            stream_proc_trace,
            // flagarray: [].as_mut(),
            cnt_array,
            a_time_array,
            write_time_array,
            circ_buff_md,
            cb_im_data,
            data_handle: mmap,
        };

        Ok(image)
    }
}

impl TryFrom<Vec<u8>> for Image<'_, Vec<u8>> {
    type Error = Error;

    fn try_from(mut data: Vec<u8>) -> Result<Self, Self::Error> {
        // so now we want to populate the data in a new IMAGE from mmap.
        // I guess either the mmap data is contiguous, or the IMAGE data is
        // contiguous, not both. So perhaps the IMAGE data is all simply cloned
        // from the SHM, including pointers etc.s

        // first up is image metadata.
        let (chunk, leftover) = data.split_at_mut(round_up_8(size_of::<ImageMetadata>()));
        let md: &mut ImageMetadata = unsafe {
            chunk
                .as_mut_ptr()
                .cast::<ImageMetadata>()
                .as_mut_unchecked()
        };

        let md_tmp = md.clone();
        DataType::try_from(md_tmp.datatype)?;

        // image array:
        let (chunk, leftover) = leftover.split_at_mut(round_up_8(md_tmp.imdatamemsize as usize));
        let array: &mut [u8] = unsafe {
            from_raw_parts_mut(
                chunk.as_mut_ptr(),
                round_up_8(md_tmp.imdatamemsize as usize),
            )
        };
        let (chunk, leftover) = leftover.split_at_mut(round_up_8(
            size_of::<ImageKeyword>() * md_tmp.nb_kw as usize,
        ));
        let kw: &mut [ImageKeyword] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.nb_kw as usize) };

        let (chunk, leftover) =
            leftover.split_at_mut(round_up_8(size_of::<libc::sem_t>() * md_tmp.sem as usize));
        let sem_file: &mut [libc::sem_t] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.sem as usize) };

        let (chunk, leftover) = leftover.split_at_mut(round_up_8(size_of::<libc::sem_t>()));
        let sem_log: &mut libc::sem_t =
            { unsafe { chunk.as_mut_ptr().cast::<libc::sem_t>().as_mut_unchecked() } };

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
        let a_time_array: &mut [TimeSpec] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.size[2] as usize) };

        let (chunk, leftover) =
            leftover.split_at_mut(round_up_8(size_of::<TimeSpec>() * md_tmp.size[2] as usize));
        let write_time_array: &mut [TimeSpec] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.size[2] as usize) };

        let (chunk, leftover) =
            leftover.split_at_mut(round_up_8(size_of::<u64>() * md_tmp.size[2] as usize));
        let cnt_array: &mut [u64] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.size[2] as usize) };

        let (chunk, leftover) = leftover.split_at_mut(round_up_8(
            size_of::<CBFrameMetadata>() * md_tmp.cb_size as usize,
        ));
        let circ_buff_md: &mut [CBFrameMetadata] =
            unsafe { from_raw_parts_mut(chunk.as_mut_ptr().cast(), md_tmp.cb_size as usize) };

        let (chunk, leftover) = leftover.split_at_mut(round_up_8(
            md_tmp.imdatamemsize as usize * md_tmp.cb_size as usize,
        ));
        let cb_im_data: &mut [u8] = unsafe {
            from_raw_parts_mut(
                chunk.as_mut_ptr(),
                round_up_8(md_tmp.imdatamemsize as usize * md_tmp.cb_size as usize),
            )
        };

        // should be nothing left in mmap:
        assert_eq!(leftover.len(), 0);

        let image = Self {
            name: md_tmp.name,
            used: 1,
            create_cnt: 1,
            sem_log,
            md,
            array,
            // // semptr,
            kw,
            sem_file,
            sem_read_pid,
            sem_write_pid,
            sem_ctrl,
            sem_status,
            stream_proc_trace,
            // flagarray: [].as_mut(),
            cnt_array,
            a_time_array,
            write_time_array,
            circ_buff_md,
            cb_im_data,
            data_handle: data,
        };

        Ok(image)
    }
}

impl TryFrom<ImageMetadata> for Image<'_, Vec<u8>> {
    type Error = Error;

    fn try_from(md: ImageMetadata) -> Result<Self, Error> {
        let ImageMetadata {
            size,
            sem,
            nb_proc_trace,
            cb_size,
            imdatamemsize,
            nb_kw,
            ..
        } = md.clone();

        let mut data = vec![];

        // metadata
        data.extend(md.to_bytes());

        // array
        data.extend(vec![0; imdatamemsize as usize]);

        // array padding
        data.extend(vec![
            0;
            round_up_8(imdatamemsize as usize)
                - imdatamemsize as usize
        ]);

        // keywords
        data.extend(vec![ImageKeyword::new(); nb_kw as usize].to_bytes()); // keywords

        // semfile
        data.extend(vec![Sem::new(); sem as usize].to_bytes());

        // semlog
        data.extend(Sem::new().to_bytes());

        // sem_read_pid
        let sem_read_pid_bytes = vec![SemPid::new(); sem as usize].to_bytes();
        let len = sem_read_pid_bytes.len();
        data.extend(sem_read_pid_bytes);
        data.extend(vec![0u8; round_up_8(len) - len]); // pad if necessary

        // sem_write_pid
        let sem_write_pid_bytes = vec![SemPid::new(); sem as usize].to_bytes();
        let len = sem_write_pid_bytes.len();
        data.extend(sem_write_pid_bytes);
        data.extend(vec![0u8; round_up_8(len) - len]); // pad if necessary

        // sem_ctrl
        let sem_ctrl_bytes = vec![SemCtrl::new(); sem as usize].to_bytes();
        let len = sem_ctrl_bytes.len();
        data.extend(sem_ctrl_bytes);
        data.extend(vec![0u8; round_up_8(len) - len]); // pad if necessary

        // sem_status
        let sem_status_bytes = vec![SemPid::new(); sem as usize].to_bytes();
        let len = sem_status_bytes.len();
        data.extend(sem_status_bytes);
        data.extend(vec![0u8; round_up_8(len) - len]); // pad if necessary

        // stream_proc_trace
        data.extend(vec![StreamProcTrace::new(); nb_proc_trace as usize].to_bytes());

        // atimearray
        data.extend(vec![TimeSpec::new(); size[2] as usize].to_bytes());

        // writetimearray
        data.extend(vec![TimeSpec::new(); size[2] as usize].to_bytes());

        // cntarray
        data.extend(vec![Cnt::new(); size[2] as usize].to_bytes());

        // circ buff metadata
        data.extend(vec![CBFrameMetadata::new(); cb_size as usize].to_bytes());

        // cb imdata
        let cb_imdatamemsize = imdatamemsize as usize * cb_size as usize;
        data.extend(vec![0; cb_imdatamemsize]);

        // cb imdata padding
        data.extend(vec![0; round_up_8(cb_imdatamemsize) - cb_imdatamemsize]);

        data.try_into()
    }
}
