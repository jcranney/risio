use std::thread;

use anyhow::Result;
use risio::Image;

fn main() -> Result<()> {
    let name = "aaaabbbbcccc";
    let mut image = Image::<u8>::read_or_create(name, &[3, 3])?;
    println!("MAIN: opened image in main thread");
    let t = thread::spawn(|| {
            let mut image = Image::<u8>::read_or_create(name, &[3, 3]).unwrap();
            println!("THREAD: opened image in spawned thread");
            image.semflush(0).unwrap();
            println!("THREAD: flushed all sems");
            println!("THREAD: waiting for sempost");
            image.semwait(0).unwrap();
            println!("THREAD: got a sem update!");
    });
    println!("MAIN: spawned thread");
    println!("MAIN: sleeping in main thread");
    std::thread::sleep(std::time::Duration::from_secs(3));
    println!("MAIN: done sleeping");
    image.sempost(0).unwrap();
    println!("MAIN: posted sem ...");
    t.join().unwrap();
    
    for (_, element) in image.array().iter_mut().enumerate() {
        *element = 0xFA;
    }
    Ok(())
}
