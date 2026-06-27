use risio::shm::RisioShm;

fn main() {
    loop {
        a();
        b();
        c();
    }
}

struct A {}
impl RisioShm for A {
    fn name(&self) -> &str {
        "heressomeshmfortesting"
    }
}

fn a() {
    let a = A {};
    println!("creating");
    a.create().unwrap();
    println!("unlinking");
    a.unlink().unwrap();
    println!("opening");
    a.open().unwrap_err();
}

fn b() {
    let a = A {};
    println!("creating");
    a.create().unwrap();
    println!("unlinking");
    a.unlink().unwrap();
    println!("creating");
    a.create().unwrap();
    println!("opening");
    a.open().unwrap();
}

fn c() {
    let a = A {};
    a.create().unwrap();
}
