use risio::{Accessor, RawImage};

fn main() -> anyhow::Result<()> {
    let mut image = RawImage::create_new::<u64>("myimname", &[10, 12])?;
    // let mut image: RawImage<u64> = RawImage::open("myimname")?;
    let x = match image.array_mut() {
        Ok(risio::ShmimMutSlice::U64(x)) => x,
        _ => panic!()
    };
    println!("{}", x[0]);
    for y in std::io::stdin().lines().into_iter() {
        x[0] = y?.parse()?;
        println!("{}", x[0]);
    }
    Ok(())
}
