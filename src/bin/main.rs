use risio::{Image, ImageType, ValidImage};

fn main() {
    // let mut image = Image::<f64>::read_sharedmem_image("shmim_from_rust", &[10, 10]).unwrap();
    // // image.array()[0] += 0.1;
    // image.array()[1] += 0.1;
    // println!("{:?}", image.array());
    // println!("hello risio!");

    let mut image = Image::<f64>::create_image(
        "shmim_from_rust",
        2,
        &[10, 10],
        -1,
        true,
        0,
        10,
        ImageType::default(),
        1,
    )
    .unwrap();
}
