use risio::{Accessor, ShmImage, error::Error};

fn main() -> Result<(), Error> {
    let mut image: ShmImage<u64> = ShmImage::create_new("myimname", &[10, 12])?;
    println!("{}", unsafe { image.array()[0] });
    for x in std::io::stdin().lines().into_iter() {
        match x?.parse() {
            Ok(v) => unsafe { image.modify(|(_, x)| *x = v) }?,
            Err(e) => {
                println!("{}", e);
                continue;
            }
        }
        println!("{}", unsafe { image.array()[0] });
    }
    Ok(())
}
