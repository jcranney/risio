use risio::{Accessor, RawImage};

#[test]
fn main() {
    let mut image: RawImage = match RawImage::open("myimname") {
        Ok(img) => img,
        Err(_) => RawImage::create_new::<u64>("myimname", &[10, 12]).unwrap(),
    };
    let x = match image.array_mut(){
        Ok(x) => match x {
            risio::ShmimMutSlice::U64(x) => x,
            _ => panic!(),
        },
        Err(e) => panic!("{}", e.to_string()),
    };
    // to get here, x must be of type u64
    x[0] = (x[0] + 7) % 6;
}