fn main() {
    let build_path = cmake::build("./libmilk");
    
    // Use the following commands to link to shared libraries
    println!("cargo:rustc-link-search={}/milk-1.03.00/lib", build_path.display()); // directory
    println!("cargo:rustc-link-lib=static=ImageStreamIO");
    println!("cargo:rustc-link-lib=static=milkCOREMODarith");
    println!("cargo:rustc-link-lib=static=milkCOREMODiofits");
    println!("cargo:rustc-link-lib=static=milkCOREMODmemory");
    println!("cargo:rustc-link-lib=static=milkCOREMODtools");
    
    // Generate bindings with bindgen
    let bindings = bindgen::Builder::default()
        .clang_arg("-Ilibmilk/src")
        .header("./libmilk/src/CommandLineInterface/CLIcore.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");
    // Write bindings to bindings.rs
    let out_path = std::path::PathBuf::from("./src");
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}