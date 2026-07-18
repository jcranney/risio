use risio::{Accessor, ShmImage, error::Error};

#[test]
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
    let posts = 100;
    // semaphore can be any value beforehand ...
    unsafe { image.sem_flush(0) };
    // ... but after flush it should be exactly 0
    let sem_init_val = unsafe { image.sem_val(0) };
    assert_eq!(sem_init_val, 0);
    
    // post to the semaphore a bunch of times
    for _ in 0..posts {
        unsafe { image.sem_post_one(0) };
    }
    
    // the semaphore value should be equal to it's initial value, plus 
    // the number of times it was posted.
    assert_eq!(unsafe { image.sem_val(0) }, sem_init_val + posts);

    // flush it again to make sure it can be reduced to 0
    unsafe { image.sem_flush(0) };
    assert_eq!(unsafe { image.sem_val(0) }, 0);

    Ok(())
}
