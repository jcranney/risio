use criterion::{Criterion, criterion_group, criterion_main};
use rayon::{
    self,
    iter::{IntoParallelRefMutIterator, ParallelIterator},
};
use std::time::Instant;

use risio::{Accessor, RawImage};

const IMNAME: &str = "benchy";
const IMSHAPE: &[usize; 2] = &[1000, 1200];

fn par_modify_image(array: &mut [f64]) {
    array.par_iter_mut().for_each(|x| {
        *x = (*x + 42.0) % 41.0;
    });
}

fn modify_image<'a>(array: &mut [f64]) {
    array.iter_mut().for_each(|x| {
        *x = (*x + 42.0) % 41.0;
    });
}

fn bench_modify_image(c: &mut Criterion) {
    let mut group = c.benchmark_group("modify image");

    group.bench_function("serial", move |b| {
        b.iter_custom(|iters| {
            let mut image = match RawImage::<f64>::open(IMNAME) {
                Ok(img) => img,
                Err(_) => {
                    // couldn't open it, we can try to create it
                    RawImage::create_new(IMNAME, IMSHAPE).unwrap()
                }
            };
            let array = unsafe { image.array_mut() };
            let start = Instant::now();
            for _ in 0..iters {
                modify_image(array);
            }
            start.elapsed()
        })
    });

    group.bench_function("rayon", move |b| {
        b.iter_custom(|iters| {
            let mut image = match RawImage::<f64>::open(IMNAME) {
                Ok(img) => img,
                Err(_) => {
                    // couldn't open it, we can try to create it
                    RawImage::create_new(IMNAME, IMSHAPE).unwrap()
                }
            };
            let array = unsafe { image.array_mut() };
            let start = Instant::now();
            for _ in 0..iters {
                par_modify_image(array);
            }
            start.elapsed()
        })
    });
}

criterion_group!(benches, bench_modify_image);
criterion_main!(benches);
