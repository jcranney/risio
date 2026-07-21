use criterion::{Criterion, criterion_group, criterion_main};
use memmap2::MmapMut;
use rayon::{
    self,
    iter::{IntoParallelRefMutIterator, ParallelIterator},
};
use std::time::Instant;

use risio::{Accessor, ShmImage};

const IMNAME: &str = "benchy";
const IMSHAPE: &[usize; 2] = &[1000, 1200];

fn par_modify_image<'a, T>(image: &mut T)
where
    T: Accessor<'a, MmapMut, DTYPE = f64>,
{
    unsafe {
        image
            .par_modify(|(_, x)| {
                *x = (*x + 43.0) % T::DTYPE::from(42.0);
            })
            .unwrap()
    };
}

fn modify_image<'a, T>(image: &mut T)
where
    T: Accessor<'a, MmapMut, DTYPE = f64>,
{
    unsafe {
        image
            .modify(|(_, x)| {
                *x = (*x + 43.0) % T::DTYPE::from(42.0);
            })
            .unwrap()
    };
}

fn par_iter_mut_array(array: &mut [f64]) {
    array.par_iter_mut().for_each(|x| {
        *x = (*x + 43.0) % 42.0;
    });
}

fn iter_mut_array(array: &mut [f64]) {
    array.iter_mut().for_each(|x| {
        *x = (*x + 43.0) % 42.0;
    });
}

fn bench_modify_image(c: &mut Criterion) {
    let mut group = c.benchmark_group("modify image");
    group.bench_function("modify serial", move |b| {
        b.iter_batched_ref(
            || {
                match ShmImage::<f64>::open(IMNAME) {
                    Ok(img) => img,
                    Err(_) => {
                        // couldn't open it, we can try to create it
                        ShmImage::create_new(IMNAME, IMSHAPE).unwrap()
                    }
                }
            },
            |im| modify_image(im),
            criterion::BatchSize::LargeInput,
        )
    });
    group.bench_function("modify rayon", move |b| {
        b.iter_batched_ref(
            || {
                match ShmImage::<f64>::open(IMNAME) {
                    Ok(img) => img,
                    Err(_) => {
                        // couldn't open it, we can try to create it
                        ShmImage::create_new(IMNAME, IMSHAPE).unwrap()
                    }
                }
            },
            |im| par_modify_image(im),
            criterion::BatchSize::LargeInput,
        )
    });

    group.bench_function("array serial", move |b| {
        b.iter_custom(|iters| {
            let mut image = match ShmImage::<f64>::open(IMNAME) {
                Ok(img) => img,
                Err(_) => {
                    // couldn't open it, we can try to create it
                    ShmImage::create_new(IMNAME, IMSHAPE).unwrap()
                }
            };
            let array = unsafe { image.array_mut() };
            let start = Instant::now();
            for _ in 0..iters {
                iter_mut_array(array);
            }
            start.elapsed()
        })
    });

    group.bench_function("array rayon", move |b| {
        b.iter_custom(|iters| {
            let mut image = match ShmImage::<f64>::open(IMNAME) {
                Ok(img) => img,
                Err(_) => {
                    // couldn't open it, we can try to create it
                    ShmImage::create_new(IMNAME, IMSHAPE).unwrap()
                }
            };
            let array = unsafe { image.array_mut() };
            let start = Instant::now();
            for _ in 0..iters {
                par_iter_mut_array(array);
            }
            start.elapsed()
        })
    });
}

criterion_group!(benches, bench_modify_image);
criterion_main!(benches);
