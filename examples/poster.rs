use std::time::Duration;
use rand::random;
use risio::{Accessor, ShmImage, error::Error};

fn main() -> Result<(), Error> {
    let mut image: ShmImage<f64> = match ShmImage::create_new("noise", &[100, 100]) {
        Ok(x) => {
            println!("created new image!");
            x
        }
        Err(_) => {
            println!("opened existing image");
            ShmImage::open("noise")?
        }
    };
    let mut cnt: usize = 0;
    loop {
        cnt += 1;
        (unsafe { image.modify(|(_, x)| *x = random()) })?;
        unsafe { image.sem_post_one(0) };
        println!("semval: {}", unsafe { image.sem_val(0) });
        println!("{cnt}: posted!");
        std::thread::sleep(Duration::from_millis(100));
    }
}
