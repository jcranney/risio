// Create the shared memory object.

// use crate::bindings::IMAGE;
use anyhow::Result;
use std::ffi::CString;
use thiserror::Error;

use libc;

#[derive(Error, Debug)]
enum RisioSHMError {
    #[error(
        "Somehow, an error was detected (usually by a negative value being \
    returned from a libc call), but when the libc `errno` was checked it's value \
    was zero. Very odd behavious indeed."
    )]
    InvalidErrorCode,
}

pub trait RisioShm {
    fn name(&self) -> &str;

    /// `open` will attempt to open a shm image, but not create it if it's missing.
    fn open(&self) -> Result<()> {
        let fd = match unsafe { libc::shm_open(self.name_csrt()?.as_ptr(), libc::O_RDWR, 0) } {
            e if e < 0 => return Self::get_error(),
            fd => fd,
        };

        println!("fd: {}, fdfl: {}", fd, unsafe {
            libc::fcntl(fd, libc::F_GETFL)
        },);

        match unsafe { libc::close(fd) } {
            e if e < 0 => Self::get_error()?,
            _ => (),
        };

        println!("fd: {}, fdfl: {}", fd, unsafe {
            libc::fcntl(fd, libc::F_GETFL)
        },);

        Ok(())
    }

    fn name_csrt(&self) -> Result<CString> {
        let mut null_terminated: Vec<u8> = "/".as_bytes().to_vec();
        null_terminated.append(&mut self.name().as_bytes().to_vec());
        null_terminated.append(&mut ".im.shm".as_bytes().to_vec());
        null_terminated.push(0);
        Ok(CString::from_vec_with_nul(null_terminated)?)
    }

    /// Only call this function if an error has been indicated (e.g., by a libc
    /// function returning < 0. Calling this outside of such a context is
    /// undefined behaviour, and to protect the user from such a mistake,
    /// even an error id of "0" is identified as an error.
    fn get_error() -> Result<()> {
        match std::io::Error::last_os_error().raw_os_error().unwrap_or(0) {
            0 => Err(RisioSHMError::InvalidErrorCode)?,
            x => Err(std::io::Error::from_raw_os_error(x))?,
        }
    }

    /// `create` will open a (non-exclusive) shm image, creating it if necessary.
    fn create(&self) -> Result<()> {
        let fd = unsafe {
            libc::shm_open(
                self.name_csrt()?.as_ptr(),
                libc::O_CREAT | libc::O_RDWR,
                0o666,
            )
        };
        if fd < 0 {
            Self::get_error()?;
        }

        println!("fd: {}, fdfl: {}", fd, unsafe {
            libc::fcntl(fd, libc::F_GETFL)
        },);

        match unsafe { libc::close(fd) } {
            e if e < 0 => Self::get_error()?,
            _ => (),
        };

        println!("fd: {}, fdfl: {}", fd, unsafe {
            libc::fcntl(fd, libc::F_GETFL)
        },);

        // if unsafe { libc::ftruncate(fd, size_of::<IMAGE>() as i64) } < 0 {
        //     Self::get_error()?;
        // }
        // Resize the shared memory object to the size of our data.
        // rustix::fs::ftruncate(&fd, size_of::<IMAGE>() as u64)?;

        // Map the shared memory object into our address space.
        //
        // SAFETY: We're creating a new mapping that's independent of any existing
        // memory allocations. There are interesting things to say about *using*
        // `ptr`, but that's for another safety comment.
        // let ptr = unsafe {
        //     rustix::mm::mmap(
        //         null_mut(),
        //         size_of::<IMAGE>(),
        //         ProtFlags::READ | ProtFlags::WRITE,
        //         MapFlags::SHARED,
        //         &fd,
        //         0,
        //     )?
        // };
        Ok(())
    }

    /// `unlink` will remove the shm image from memory
    fn unlink(&self) -> Result<()> {
        if unsafe { libc::shm_unlink(self.name_csrt()?.as_ptr()) } < 0 {
            Self::get_error()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct A {}
    impl RisioShm for A {
        fn name(&self) -> &str {
            "heressomeshmfortesting"
        }
    }

    #[test]
    fn test_shm_open_panic() {
        let a = A {};
        println!("creating");
        a.create().unwrap();
        println!("unlinking");
        a.unlink().unwrap();
        println!("opening");
        a.open().unwrap_err();
    }

    #[test]
    fn test_shm_open() {
        let a = A {};
        println!("creating");
        a.create().unwrap();
        println!("unlinking");
        a.unlink().unwrap();
        println!("creating");
        a.create().unwrap();
        println!("opening");
        a.open().unwrap();
    }

    #[test]
    fn test_shm_create() {
        let a = A {};
        a.create().unwrap();
    }
}
