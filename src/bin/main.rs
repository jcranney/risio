use risio::Image;

fn main() {
    let image = Image::<f64>::read_or_create("shmim_from_rust", &[1000, 1000]).unwrap();
    for (_, element) in image.array().iter_mut().enumerate() {
        *element = 10.0 as f64;
    }
}
