use risio::{Accessor, RawImage};

#[test]
fn main() {
    let mut image: RawImage<u64> = match RawImage::open("myimname") {
        Ok(img) => img,
        Err(_) => RawImage::create_new("myimname", &[10, 12]).unwrap(),
    };
    let x = image.array()[0];
    let y = (x + 1) % 5;
    {
        image.array_mut()[0] = y;
    }
    // let w = image.array()[0];
    // assert_eq!(w, y);
}
