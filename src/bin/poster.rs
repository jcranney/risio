use std::time::Duration;

use rand::random;
use risio::{Accessor, RawImage, error::Error};

fn main() -> Result<(), Error> {
    // let mut image: RawImage<f64> = match RawImage::create_new("noise", &[100, 100]) {
    //     Ok(x) => {
    //         println!("created new image!");
    //         x
    //     }
    //     Err(_) => {
    //         println!("opened existing image");
    //         RawImage::open("noise")?
    //     }
    // };
    // // unsafe { libc::sem_init(image._image.semlog, 1, 1) };
    // let mut cnt: usize = 0;
    // loop {
    //     cnt += 1;
    //     image.array_mut().iter_mut().for_each(|x| *x = random());
    //     unsafe { libc::sem_post(image._image.semlog) };
    //     println!("{cnt}: posted!");
    //     std::thread::sleep(Duration::from_millis(100));
    // }
    Ok(())
}
