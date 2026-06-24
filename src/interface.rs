use std::{
    sync::mpsc::Receiver,
    thread,
    time::{Duration, Instant},
};

use crate::{Image, RisioError, ValidImageType};
use anyhow::Result;

pub enum Context {
    Initialising,
    Running,
    Exiting,
}

/// Supertrait for all tasks that can open
pub trait Opens<T: ValidImageType<T>> {
    fn name_and_shape(&self) -> (&str, &[u32]);
}

pub trait Computes<T: ValidImageType<T>>: Opens<T> {
    fn task(&mut self, shm_array: &mut [T], context: &Context) -> ();

    fn init(&mut self) -> Result<()> {
        let (name, shape) = self.name_and_shape();
        let mut image = Image::<T>::read_or_create(name, shape)?;
        let array = image.array();
        self.task(array, &Context::Initialising);
        image.sempost(0)?;
        Ok(())
    }

    fn run(&mut self) -> Result<()> {
        let (name, shape) = self.name_and_shape();
        let mut image = Image::<T>::read_or_create(name, shape)?;
        let array = image.array();
        self.task(array, &Context::Running);
        image.sempost(0)?;
        Ok(())
    }

    fn exit(&mut self) -> Result<()> {
        let (name, shape) = self.name_and_shape();
        let mut image = Image::<T>::read_or_create(name, shape)?;
        let array = image.array();
        self.task(array, &Context::Running);
        image.sempost(0)?;
        Ok(())
    }

    fn looper(&mut self, period: Duration, sig_exit: Receiver<()>) {
        self.init().unwrap();
        let mut t = Instant::now();
        loop {
            self.run().unwrap();
            if let Ok(()) = sig_exit.try_recv() {
                self.exit().unwrap();
                break;
            }
            thread::sleep(period.saturating_sub(t.elapsed()));
            t = Instant::now();
        }
    }
}

pub trait Measures<T: ValidImageType<T>>: Opens<T> {
    fn task(&mut self, shm_array: &[T], context: &Context) -> ();

    fn init(&mut self) -> Result<()> {
        let (name, shape) = self.name_and_shape();
        let mut image = Image::<T>::read_or_create(name, shape)?;
        let array: &[T] = image.array();
        self.task(array, &Context::Initialising);
        Ok(())
    }

    fn run(&mut self) -> Result<()> {
        let (name, shape) = self.name_and_shape();
        let mut image = Image::<T>::read_or_create(name, shape)?;
        let array: &[T] = image.array();
        self.task(array, &Context::Running);
        Ok(())
    }

    fn exit(&mut self) -> Result<()> {
        let (name, shape) = self.name_and_shape();
        let mut image = Image::<T>::read_or_create(name, shape)?;
        let array: &[T] = image.array();
        self.task(array, &Context::Running);
        Ok(())
    }
}

trait StochasticProcess<T: ValidImageType<T>> {
    /// inputs is a function that should return a (possibly empty) slice of tuples,
    /// with each tuple containing the SHM object name, and a slice defining the
    /// dimensions of that image. Note: the slice is currently at most 3 long.
    fn inputs(&self) -> &[(&str, &[u32])];
    fn output(&self) -> (&str, &[u32]);
    fn matrices(&self) -> &[&[&[T]]];
    fn std_matrix(&self) -> Option<&[&[T]]>;

    fn init(&self) -> Result<()> {
        todo!()
        // maybe load the inputs once here, but not sure how to keep them from
        // dropping?
    }
    
    fn update(&self) -> Result<()> {
        todo!()
        // update the output based on the current input
    }

    fn close(&self) -> Result<()> {
        todo!()
        // drop anything that isn't automatically dropped.
    }

    // fn load(inputs: &[(&str, &[u32])], output: (&str, &[u32])) -> Result<Self> {
    //     Ok(Self {
    //         matrices: vec![],
    //         inputs: inputs
    //             .iter()
    //             .map(|(name, shape)| Ok(Image::<T>::read_or_create(name, shape)?))
    //             .collect::<Result<Vec<Image<T>>>>()?,
    //         std_matrix: vec![],
    //         output: Image::<T>::read_or_create(output.0, output.1)?,
    //     })
    // }

}



impl StochasticProcess<'a, T> for <'a, T: ValidImageType<T>> {
    fn new(inputs: &[(&str, &[u32])], output: (&str, &[u32])) -> Result<Self> {
        Ok(Self {
            matrices: vec![],
            inputs: inputs
                .iter()
                .map(|(name, shape)| Ok(Image::<T>::read_or_create(name, shape)?))
                .collect::<Result<Vec<Image<T>>>>()?,
            std_matrix: vec![],
            output: Image::<T>::read_or_create(output.0, output.1)?,
        })
    }
    fn run_for_n(&self, n: usize) {
        for i in 0..n {
            println!("{}", i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_a_test() {
        assert!(true);
    }

    #[test]
    fn create_sensor() {
        let sensor = StochasticProcess::<f64>::new(
            &[("x1", &[10]), ("x2", &[12]), ("u1", &[15])],
            ("s1", &[20, 1, 1]),
        ).unwrap();
        sensor.run_for_n(100);
    }
}
