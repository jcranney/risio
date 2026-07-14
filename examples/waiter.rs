use risio::{Accessor, RawImage, error::Error};

fn main() -> Result<(), Error> {
    let mut image: RawImage<f64> = match RawImage::create_new("noise", &[100, 100]) {
        Ok(x) => {
            println!("created new image!");
            x
        }
        Err(_) => {
            println!("opened existing image");
            RawImage::open("noise")?
        }
    };
    let mut cnt: usize = 0;
    unsafe { image.sem_flush(0) };
    loop {
        cnt += 1;
        println!("semval: {}", unsafe { image.sem_val(0) });
        unsafe { image.sem_wait(0) };
        let x: f64 =
            unsafe { image.array().iter().sum::<f64>() } / unsafe { image.array().len() } as f64;
        println!("{cnt}: mean(im) = {:0.8}", x);
    }
}
