use risio::{Accessor, RawImage, error::Error};

fn main() -> Result<(), Error> {
    let mut image: RawImage<u64> = RawImage::create_new("myimname", &[10, 12])?;
    // let mut image: RawImage<u64> = RawImage::open("myimname")?;
    println!("{}", image.array()[0]);
    for x in std::io::stdin().lines().into_iter() {
        match x?.parse() {
            Ok(v) => image.array_mut()[0] = v,
            Err(e) => {
                println!("{}", e);
                continue;
            }
        }
        println!("{}", image.array()[0]);
    }
    Ok(())
}
