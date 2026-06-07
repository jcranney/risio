use risio::{Image, ImageType, ValidImage};

fn main() {
    let mut image = Image::<u8>::create_image(
        "shmim_from_rust",
        2,
        &[100, 100],
        -1,
        true,
        5,
        10,
        ImageType::default(),
        1,
    )
    .unwrap();
    // println!("{:?}", image.array.);
    println!("{:?}", image);
    image.sempost(0);
    println!("{:?}", image.array());
    image.sempost(0);
    println!("{:?}", image.array());
    println!("hello risio!");
}
