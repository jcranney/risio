use anyhow::Result;
use core::ffi::CStr;
use sem_safe::named::{OpenFlags, Semaphore};

pub trait RisioSem {
    fn name(&self) -> &CStr;

    fn wait(&self) -> Result<()> {
        let s = self.sem()?;
        Self::parse_result(s.sem_ref().wait())
    }

    fn try_wait(&self) -> Result<()> {
        let s = self.sem()?;
        Self::parse_result(s.sem_ref().try_wait())
    }

    fn post(&self) -> Result<()> {
        let s = self.sem()?;
        Self::parse_result(s.sem_ref().post())
    }

    fn sem(&self) -> Result<Semaphore> {
        Self::parse_result(Semaphore::open(self.name(), OpenFlags::AccessOnly))
    }

    fn create(&self) -> Result<()> {
        let s = Semaphore::open(
            self.name(),
            OpenFlags::Create {
                exclusive: false,
                mode: 0o666,
                value: 0,
            },
        );
        let _ = Self::parse_result(s)?;
        Ok(())
    }

    fn flush(&self) -> Result<()> {
        // modify the semaphore positively or negatively until the semaphore
        // is back to zero.
        let s = self.sem()?;
        while s.sem_ref().get_value() > 0 {
            println!("{}", s.sem_ref().get_value());
            Self::parse_result(s.sem_ref().try_wait())?;
        }
        while s.sem_ref().get_value() < 0 {
            Self::parse_result(s.sem_ref().post())?;
        }
        Ok(())
    }

    fn parse_result<T>(result: Result<T, ()>) -> Result<T> {
        match result {
            Ok(x) => Ok(x),
            Err(()) => match std::io::Error::last_os_error().raw_os_error().unwrap() {
                x => Err(std::io::Error::from_raw_os_error(x))?,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        thread::{self, JoinHandle},
        time::Duration,
    };

    use super::*;

    #[test]
    fn test_sem() {
        struct A {}
        impl RisioSem for A {
            fn name(&self) -> &std::ffi::CStr {
                c"thisisasemaphoreusedfortestingtherisiocratepleasedontuseitforconcurrency"
            }
        }
        let a = A {};
        a.create().unwrap();
        a.flush().unwrap();

        let thread: JoinHandle<Result<()>> = std::thread::spawn(|| {
            let a_in_thread = A {};
            println!("THREAD: sem wait");
            a_in_thread.wait()?;
            println!("THREAD: released!");
            Ok(())
        });

        println!("MAIN: timer wait");
        thread::sleep(Duration::from_millis(100));
        assert!(!thread.is_finished());
        a.post().unwrap();
        println!("MAIN: timer done!");
        thread::sleep(Duration::from_millis(100));
        assert!(thread.is_finished());
        thread.join().unwrap().unwrap();
    }
}
