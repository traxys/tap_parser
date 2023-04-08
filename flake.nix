{
  description = "A basic flake with a shell";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  inputs.flake-utils.url = "github:numtide/flake-utils";
  inputs.naersk.url = "github:nix-community/naersk";
  inputs.rust-overlay.url = "github:oxalica/rust-overlay";

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    naersk,
    rust-overlay,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [(import rust-overlay)];
      };
      rust = pkgs.rust-bin.stable.latest.default;
      nightly = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default);
      naersk' = pkgs.callPackage naersk {
        cargo = rust;
        rustc = rust;
      };
      cargo-fuzz-wrapped = pkgs.writeShellScriptBin "cargo-fuzz" ''
        export RUSTC="${nightly}/bin/rustc";
        export CARGO="${nightly}/bin/cargo";
        exec "${pkgs.cargo-fuzz}/bin/cargo-fuzz" "$@"
      '';
    in {
      devShell = pkgs.mkShell {
        nativeBuildInputs = [rust pkgs.cargo-tarpaulin cargo-fuzz-wrapped];
        RUST_PATH = "${rust}";
        shellHook = ''
          alias rstddoc="firefox ${rust}/share/doc/rust/html/std/index.html"
        '';
      };

      defaultPackage = naersk'.buildPackage ./.;
    });
}
