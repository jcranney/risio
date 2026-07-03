use risio::{Accessor, RawImage};

fn main() -> anyhow::Result<()> {
    let mut image: RawImage<u64> = RawImage::create_new("myimname", &[10, 12])?;
    // let mut image: RawImage<u64> = RawImage::open("myimname")?;
    println!("{}", image.array()[0]);
    for x in std::io::stdin().lines().into_iter() {
        image.array_mut()[0] = x?.parse()?;
        println!("{}", image.array()[0]);
    }
    Ok(())
}
