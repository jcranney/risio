use criterion::{Criterion, criterion_group, criterion_main};
use rayon::{
    self,
    iter::{IntoParallelRefMutIterator, ParallelIterator},
};
use std::hint::black_box;

use risio::{Accessor, RawImage};

const IMNAME: &str = "benchy";
const IMSHAPE: &[usize; 2] = &[1000, 1200];

fn par_modify_image(image: &mut RawImage) {
    match image.array_mut() {
        Ok(risio::ShmimMutSlice::F64(x)) => x.par_iter_mut().for_each(|x| {
            *x = (*x + 42.0) % 41.0;
        }),
        _ => panic!(),
    };
}

fn modify_image(image: &mut RawImage) {
    match image.array_mut() {
        Ok(risio::ShmimMutSlice::F64(x)) => x.iter_mut().for_each(|x| {
            *x = (*x + 42.0) % 41.0;
        }),
        _ => panic!(),
    };
}

fn bench_modify_image(c: &mut Criterion) {
    let mut group = c.benchmark_group("modify image");

    // open the image
    let mut image: RawImage = match RawImage::open(IMNAME) {
        Ok(img) => img,
        Err(_) => {
            // couldn't open it, we can try to create it
            RawImage::create_new::<f64>(IMNAME, IMSHAPE).unwrap()
        }
    };

    group.bench_function("serial", |b| b.iter(|| modify_image(black_box(&mut image))));
    group.bench_function("par", |b| {
        b.iter(|| par_modify_image(black_box(&mut image)))
    });
}

criterion_group!(benches, bench_modify_image);
criterion_main!(benches);
