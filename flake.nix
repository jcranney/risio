{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, naersk }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        naersk-lib = pkgs.callPackage naersk { };
      in
      {
        defaultPackage = naersk-lib.buildPackage ./.;
        devShell = with pkgs; mkShell rec {
          buildInputs = [ cargo rustc rustfmt pre-commit rustPackages.clippy
          ];
          packages = with pkgs; [
            cmake pkg-config cfitsio libclang.lib
            llvmPackages.libcxxClang
            clang rust-analyzer
            glib
            libGL
            fontconfig
            libxkbcommon
            wayland
            dbus
            freetype
          ];
          RUST_SRC_PATH = rustPlatform.rustLibSrc;
          LIBCLANG_PATH = "${libclang.lib}/lib";
          MILK_SHM_DIR = "/dev/shm";
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath packages;
        };
      }
    );
}
