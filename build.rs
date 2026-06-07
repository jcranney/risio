fn main() {
    let build_path = cmake::build("./libmilk");
    
    // Use the following commands to link to shared libraries
    println!("cargo:rustc-link-search={}/lib", build_path.display()); // directory
    println!("cargo:rustc-link-lib=static=milk");
    
    // Generate bindings with bindgen
    let bindings = bindgen::Builder::default()
        .header("libmilk/src/CommandLineInterface/IMGID.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");
    // Write bindings to bindings.rs
    let out_path = std::path::PathBuf::from("./src");
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}