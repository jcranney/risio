use anyhow::Result;
use pyo3::{
    exceptions::{PyIndexError, PyRuntimeError},
    prelude::*,
};
use std::{
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::ValidImageType;

#[pymodule]
#[pyo3(name = "risio")]
fn risio(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ImageF64>()?;
    Ok(())
}

#[pyclass]
struct ImageF64 {
    thread: ImageThread<f64>,
}

#[pymethods]
impl ImageF64 {
    #[new]
    fn new(name: &str, shape: Vec<i32>) -> PyResult<Self> {
        let thread = ImageThread::new(name, shape)?;
        thread.block();
        Ok(Self { thread })
    }

    fn write(&mut self, data: Vec<f64>) -> PyResult<()> {
        self.block();
        if self.thread.thread_write.is_finished() {
            // return self.thread.join();
            return Err(PyRuntimeError::new_err(
                "Image thread finished before writing",
            ));
        } else {
            self.thread.data_tx.blocking_send(data).map_err(|e| {
                PyRuntimeError::new_err(format!(
                    "data receiver has already been deallocated \
                    unexpected behaviour. Raise an Issue. {}",
                    e
                ))
            })?
        }
        self.block();
        Ok(())
    }

    fn read(&self) -> PyResult<Vec<f64>> {
        self.block();
        if self.thread.thread_read.is_finished() {
            // return self.thread.join();
            Err(PyRuntimeError::new_err(
                "Image thread finished before reading",
            ))
        } else {
            if let Some(data) = self.thread.data_rx.borrow().clone() {
                self.block();
                Ok(data.to_vec())
            } else {
                Err(PyRuntimeError::new_err("Data hasn't yet been initialised."))
            }
        }
    }

    fn rw_status(&self) -> PyResult<(ThreadState, ThreadState)> {
        Ok(self.thread.rw_status())
    }

    fn block(&self) {
        self.thread.block();

    }
}

struct ImageThread<T> {
    thread_read: JoinHandle<Result<()>>,
    thread_write: JoinHandle<Result<()>>,
    data_tx: tokio::sync::mpsc::Sender<Vec<T>>,
    data_rx: tokio::sync::watch::Receiver<Option<Vec<T>>>,
    status_read: Arc<Mutex<ThreadState>>,
    status_write: Arc<Mutex<ThreadState>>,
    mutex: Arc<Mutex<()>>,
}

impl<T: Clone + Send + Sync + ValidImageType<T> + 'static> ImageThread<T> {
    fn new(name: &str, shape: Vec<i32>) -> PyResult<Self> {
        // connect to an image, spawn a thread that owns it, and provide a
        // method for the python user to read/write to it
        if shape.iter().any(|x| *x <= 0) {
            return Err(PyIndexError::new_err("all shape indices must be positive"));
        }
        let shape_read: Vec<u32> = shape.into_iter().map(|x| x as u32).collect();
        let shape_write: Vec<u32> = shape_read.clone();
        let name_read = name.to_owned();
        let name_write = name.to_owned();
        let (tx_main, mut rx_thread) = tokio::sync::mpsc::channel::<Vec<T>>(1);
        let (tx_thread, rx_main) = tokio::sync::watch::channel::<Option<Vec<T>>>(None);
        let status_read = Arc::new(Mutex::new(ThreadState::Initialising));
        let status_write = Arc::new(Mutex::new(ThreadState::Initialising));
        // let (tx_status, rx_status) = std::sync::mpsc::channel::<ThreadStatus>();
        let status_read_thread = status_read.clone();
        let status_write_thread = status_write.clone();
        let mutex = Arc::new(Mutex::new(()));

        // some inconsistent behavious, so I'll try two threads - one for
        // reading and one for writing.
        let mutex_write = mutex.clone();
        let mutex_read = mutex.clone();
        let t_write = thread::spawn::<_, Result<()>>(move || {
            // tx_status.send(ThreadStatus::Initialising)?;
            {
                let mut status = status_write_thread.lock().unwrap();
                *status = ThreadState::Initialising;
            }
            let mut image;
            match crate::Image::<T>::open_or_create(&name_write, &shape_write) {
                Ok(im) => {
                    image = im;
                }
                Err(e) => {
                    let mut status = status_write_thread.lock().unwrap();
                    *status = ThreadState::Finished;
                    return Err(e)?;
                }
            }
            loop {
                {
                    *status_write_thread.lock().unwrap() = ThreadState::Ready;
                }
                match rx_thread.blocking_recv() {
                    Some(data) => {
                        {
                            let mut status = status_write_thread.lock().unwrap();
                            *status = ThreadState::Updating;
                        }
                        let guard = mutex_write.lock().unwrap();
                        image.array().iter_mut().zip(data).for_each(|(o, i)| *o = i);
                        image.sempost(0)?;
                        drop(guard);
                    }
                    None => break,
                }
            }
            {
                let mut status = status_write_thread.lock().unwrap();
                *status = ThreadState::Finished;
            }
            Ok(())
        });

        let t_read = thread::spawn::<_, Result<()>>(move || {
            // tx_status.send(ThreadStatus::Initialising)?;
            {
                let mut status = status_read_thread.lock().unwrap();
                *status = ThreadState::Initialising;
            }
            let mut image;
            match crate::Image::<T>::open_or_create(&name_read, &shape_read) {
                Ok(im) => {
                    image = im;
                    let guard = mutex_read.lock().unwrap();
                    tx_thread.send(Some(image.array().to_vec()))?;
                    drop(guard);
                }
                Err(e) => {
                    let mut status = status_read_thread.lock().unwrap();
                    *status = ThreadState::Finished;
                    return Err(e)?;
                }
            }
            loop {
                {
                    let mut status = status_read_thread.lock().unwrap();
                    *status = ThreadState::Ready;
                }
                image.semwait(0)?;
                {
                    let mut status = status_read_thread.lock().unwrap();
                    *status = ThreadState::Updating;
                }
                let guard = mutex_read.lock().unwrap();
                tx_thread
                    .send_replace(Some(image.array().to_vec()))
                    .unwrap();
                drop(guard);
            }
        });
        // wait for both threads to finish initialising
        let new = Self {
            thread_read: t_read,
            thread_write: t_write,
            data_tx: tx_main,
            data_rx: rx_main,
            status_read,
            status_write,
            mutex,
        };

        loop {
            match new.rw_status() {
                (ThreadState::Initialising, _) => thread::sleep(Duration::from_millis(10)),
                (_, ThreadState::Initialising) => thread::sleep(Duration::from_millis(10)),
                _ => break,
            }
        }

        Ok(new)
    }

    fn rw_status(&self) -> (ThreadState, ThreadState) {
        let status_read;
        {
            let thread_status = self.status_read.lock().unwrap();
            status_read = *thread_status;
        }
        let status_write;
        {
            let thread_status = self.status_write.lock().unwrap();
            status_write = *thread_status;
        }
        (status_read, status_write)
    }

    fn block(&self) {
        let guard = self.mutex.lock().unwrap();
        drop(guard);
    }
}

#[derive(Clone, Copy)]
#[pyclass(from_py_object)]
enum ThreadState {
    Initialising,
    Ready,
    Updating,
    Finished,
}

impl std::fmt::Display for ThreadState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ThreadState::Initialising => "Initialising",
            ThreadState::Ready => "Ready",
            ThreadState::Updating => "Updating",
            ThreadState::Finished => "Finished",
        })
    }
}
