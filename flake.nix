{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
    self.submodules = true;
  };

  outputs = { self, nixpkgs, utils, naersk }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { 
          inherit system;
          config.allowUnfree = true;
        };
        naersk-lib = pkgs.callPackage naersk { };
      in
      {
        defaultPackage = naersk-lib.buildPackage {
          src = ./.;
          buildInputs = with pkgs; [ cmake pkg-config ];
          gitSubmodules = true;
          gitAllRefs = true;
          preBuild = ''
            ls
            find \
                  -name CMakeCache.txt \
                  -exec rm {} \;
            '';
        };
        devShell = with pkgs; mkShell rec {
          packages = with pkgs; [
            cargo rustc clippy cargo-watch
            cmake pkg-config cfitsio libclang.lib
            llvmPackages.libcxxClang llvmPackages.clangUseLLVM
            rustfmt cargo-flamegraph
            cargo-machete gnuplot
            rust-analyzer gcc
            glib
            libGL
            fontconfig
            libxkbcommon
            wayland
            dbus
            freetype
            gsl
            fftw
            fftwFloat
            ncurses
            readline
            bison
            flex
            openblas
          ];
          RUST_SRC_PATH = rustPlatform.rustLibSrc;
          LIBCLANG_PATH = "${libclang.lib}/lib";
          MILK_SHM_DIR = "/dev/shm";
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath packages;
        };
      }
    );
}
