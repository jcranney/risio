fn main() {
    // let build_path = cmake::build("./libImageStreamIO");
    
    // Use the following commands to link to shared libraries
    // println!("cargo:rustc-link-search={}/lib", build_path.display()); // directory
    // println!("cargo:rustc-link-lib=static=ImageStreamIO");
    
    // // Generate bindings with bindgen
    // let bindings = bindgen::Builder::default()
    //     .clang_arg(format!("-L/{}", build_path.display()))
    //     .header("./libImageStreamIO/ImageStreamIO.h")
    //     .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
    //     .generate()
    //     .expect("Unable to generate bindings");
    // // Write bindings to bindings.rs
    // let out_path = std::path::PathBuf::from("./src");
    // bindings
    //     .write_to_file(out_path.join("bindings.rs"))
    //     .expect("Couldn't write bindings!");
}