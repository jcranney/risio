# This is based on viper's article https://ayats.org/blog/nix-rustup

{
  description = "Minimal starting project for nix-based maturin package development";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, utils, rust-overlay, ... }@inputs: {
    overlays.default = final: prev: {
      pythonPackagesExtensions = prev.pythonPackagesExtensions ++ [
        (py-final: py-prev: {
          maturin-basics = py-final.callPackage ./nix/pkgs/self {};
        })
      ];
    };
  } // utils.lib.eachDefaultSystem (system: {
    # The main development environment
    devShells.default =
      let pkgs = import nixpkgs {
            inherit system;

            overlays = [
              rust-overlay.overlays.default
            ];
          };

          toolchain = pkgs.rust-bin.fromRustupToolchainFile ./toolchain.toml;

      in pkgs.mkShell rec {
        name = "risio-dev";

        shellHook = ''
          export PS1="[${name}]$ "
          echo " "
        '';

        packages = [
          toolchain
          pkgs.rust-analyzer-unwrapped
          pkgs.maturin
          pkgs.uv
        ];

        env = 
          {
            RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
          };
      };
  });
}
