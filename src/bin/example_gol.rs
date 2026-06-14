use std::{thread, time::Duration};

use anyhow::Result;
use risio::Image;

const GOL_NAME: &str = "game_of_life";
const GOL_SIZE: &[u32] = &[20, 20];

fn main() -> Result<()> {
    // initialise game:
    let image = Image::<u8>::read_or_create(GOL_NAME, GOL_SIZE).unwrap();
    image.array().iter_mut().for_each(|x| {
        *x = 0;
    });
    let updater = thread::spawn(|| gol_update);
    thread::sleep(Duration::from_secs(10));
    updater.join().unwrap();
    Ok(())
}

fn gol_update(playing: &bool) {
    let mut image = Image::<u8>::read_or_create(GOL_NAME, GOL_SIZE).unwrap();
    image.semflush(0).unwrap();
    while *playing {
        for i in 0..GOL_SIZE[0] {
            for j in 0..GOL_SIZE[1] {
                image.array()[(i*GOL_SIZE[0]+j) as usize] += 1;
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
}
