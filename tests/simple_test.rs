use risio::{Accessor, ShmImage};

#[test]
fn main() {
    let mut image: ShmImage<u64> = match ShmImage::open("myimname") {
        Ok(img) => img,
        Err(_) => ShmImage::create_new("myimname", &[10, 12]).unwrap(),
    };
    let array = unsafe { image.array() };
    let y = array[0];
    unsafe { image
        .modify(|(_, x)| {
            *x = (*x + 1) % 5;
        })
        .unwrap() };
    let array = unsafe { image.array() };
    let z = array[0];
    assert_eq!(z, (y + 1) % 5);
}
