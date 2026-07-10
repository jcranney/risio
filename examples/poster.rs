use std::time::Duration;
use rand::random;
use risio::{Accessor, RawImage, error::Error};

fn main() -> Result<(), Error> {
    let image: RawImage<f64> = match RawImage::create_new("noise", &[100, 100]) {
        Ok(x) => {
            println!("created new image!");
            x
        }
        Err(_) => {
            println!("opened existing image");
            RawImage::open("noise")?
        }
    };
    // unsafe { libc::sem_init(image._image.semlog, 1, 1) };
    let mut cnt: usize = 0;
    // let array = image.array_mut();
    // unsafe { libc::sem_post(image._image.sem_log.get().read()) };

    loop {
        cnt += 1;
        (unsafe { image.modify(|x| *x = random()) })?;
        unsafe { image.sem_post(0) };
        println!("{cnt}: posted!");
        std::thread::sleep(Duration::from_millis(100));
    }
}
