use risio::{Accessor, RawImage, error::Error};

fn main() -> Result<(), Error> {
    // let image: RawImage<f64> = match RawImage::create_new("noise", &[100, 100]) {
    //     Ok(x) => {
    //         println!("created new image!");
    //         x
    //     }
    //     Err(_) => {
    //         println!("opened existing image");
    //         RawImage::open("noise")?
    //     }
    // };
    // let mut cnt: usize = 0;
    // loop {
    //     cnt += 1;
    //     unsafe { libc::sem_wait(image._image.semlog) };
    //     let x = image.array().iter().sum::<f64>() / image.array().len() as f64;
    //     println!("{cnt}: mean(im) = {:0.8}", x);
    // }
    Ok(())
}
