use risio::{Accessor, RawImage};

#[test]
fn main() {
    let image: RawImage<u64> = match RawImage::open("myimname") {
        Ok(img) => img,
        Err(_) => RawImage::create_new("myimname", &[10, 12]).unwrap(),
    };
    let array = unsafe { image.array() };
    let y = array[0];
    unsafe { image
        .modify(|(_, x)| {
            *x = (*x + 1) % 5;
        })
        .unwrap() };
    let z = array[0];
    assert_eq!(z, (y + 1) % 5);
}
