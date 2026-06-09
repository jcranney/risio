use risio::{DataType, Image, ImageType, ValidImage, bindings::IMGID};

fn main() {
    // let mut image = Image::<f64>::read_sharedmem_image("shmim_from_rust", &[10, 10]).unwrap();
    // // image.array()[0] += 0.1;
    // image.array()[1] += 0.1;
    // println!("{:?}", image.array());
    // println!("hello risio!");
    let mut img = unsafe {
        IMGID::stream_connect_create_2D("shmim_from_rust", 1000, 1000, DataType::F64).unwrap()
    };
    let array = img.array_f64();
    for (i, element) in array.iter_mut().enumerate() {
        *element += i as f64;
    }
}
