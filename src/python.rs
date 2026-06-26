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
        thread::sleep(Duration::from_millis(100)); // Still not really sure why this is needed but there are validity issues if I don't have it.
        Ok(Self { thread })
    }

    fn write(&mut self, data: Vec<f64>) -> PyResult<()> {
        if self.thread.thread_write.is_finished() {
            return Err(PyRuntimeError::new_err(
                "Image thread finished before writing",
            ));
        } else {
            self.thread.data_tx.send(data).map_err(|e| {
                PyRuntimeError::new_err(format!(
                    "data receiver has already been deallocated \
                    unexpected behaviour. Raise an Issue. {}",
                    e
                ))
            })?;
        }
        Ok(())
    }

    fn read(&mut self) -> PyResult<Vec<f64>> {
        if self.thread.thread_read.is_finished() {
            // return self.thread.join();
            Err(PyRuntimeError::new_err(
                "Image thread finished before reading",
            ))
        } else {
            Ok((*self.thread.data_read.lock().unwrap()).clone())
        }
    }
}

struct ImageThread<T> {
    thread_read: JoinHandle<Result<()>>,
    thread_write: JoinHandle<Result<()>>,
    data_tx: std::sync::mpsc::Sender<Vec<T>>,
    data_read: Arc<Mutex<Vec<T>>>,
    exit_tx_read: std::sync::mpsc::Sender<()>,
    exit_tx_write: std::sync::mpsc::Sender<()>,
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

        let data: Arc<Mutex<Vec<T>>> = Arc::new(Mutex::new(vec![]));
        let data_read_thread = data.clone();
        let data_write_thread = data.clone();

        let (tx_main, rx_thread) = std::sync::mpsc::channel::<Vec<T>>();
        let (tx_ready, rx_ready) = std::sync::mpsc::channel();
        let (tx_write_exit, rx_write_exit) = std::sync::mpsc::channel::<()>();
        let t_write = thread::spawn(move || {
            let mut image;
            match crate::Image::<T>::open_or_create(&name_write, &shape_write) {
                Ok(im) => {
                    image = im;
                }
                Err(e) => {
                    return Err(e)?;
                }
            }
            tx_ready.send(()).unwrap();
            loop {
                match rx_thread.try_recv() {
                    Ok(data) => {
                        let guard = data_write_thread.lock().unwrap();
                        image.array().iter_mut().zip(data).for_each(|(o, i)| *o = i);
                        image.sempost(-1)?;
                        drop(guard);
                    }
                    Err(e) => match e {
                        std::sync::mpsc::TryRecvError::Empty => (),
                        std::sync::mpsc::TryRecvError::Disconnected => break,
                    },
                }
                match rx_write_exit.try_recv() {
                    Ok(()) => break,
                    Err(e) => match e {
                        std::sync::mpsc::TryRecvError::Empty => continue,
                        std::sync::mpsc::TryRecvError::Disconnected => break,
                    },
                }
            }
            Ok(())
        });
        rx_ready.recv().unwrap();

        let (tx_ready, rx_ready) = std::sync::mpsc::channel();
        let (tx_read_exit, rx_read_exit) = std::sync::mpsc::channel::<()>();
        let t_read = thread::spawn::<_, Result<()>>(move || {
            let mut image;
            match crate::Image::<T>::open_or_create(&name_read, &shape_read) {
                Ok(im) => {
                    image = im;
                }
                Err(e) => {
                    return Err(e)?;
                }
            }
            {
                *data_read_thread.lock().unwrap() = image.array().to_vec();
            }
            image.semflush(-1)?;
            tx_ready.send(()).unwrap();
            loop {
                match image.semtrywait(0)? {
                    Some(()) => {
                        *data_read_thread.lock().unwrap() = image.array().to_vec();
                    }
                    None => (),
                }
                match rx_read_exit.try_recv() {
                    Ok(()) => break,
                    Err(e) => match e {
                        std::sync::mpsc::TryRecvError::Empty => continue,
                        std::sync::mpsc::TryRecvError::Disconnected => break,
                    },
                }
            }
            Ok(())
        });
        rx_ready.recv().unwrap();

        let new = Self {
            thread_read: t_read,
            thread_write: t_write,
            data_tx: tx_main,
            data_read: data,
            exit_tx_read: tx_read_exit,
            exit_tx_write: tx_write_exit,
        };

        Ok(new)
    }
}

impl<T> Drop for ImageThread<T> {
    fn drop(&mut self) {
        let _ = self.exit_tx_read.send(());
        let _ = self.exit_tx_write.send(());
        while !self.thread_read.is_finished(){};
        while !self.thread_write.is_finished(){};
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
