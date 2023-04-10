{
  description = "A basic flake with a shell";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  inputs.flake-utils.url = "github:numtide/flake-utils";
  inputs.naersk.url = "github:nix-community/naersk";
  inputs.rust-overlay.url = "github:oxalica/rust-overlay";
  inputs.pre-commit-hooks.url = "github:cachix/pre-commit-hooks.nix";

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    naersk,
    rust-overlay,
    pre-commit-hooks,
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
      checks = {
        pre-commit-check = pre-commit-hooks.lib.${system}.run {
          src = ./.;
          hooks = {
            rustfmt.enable = true;
          };
        };
      };

      devShell = pkgs.mkShell {
        inherit (self.checks.${system}.pre-commit-check) shellHook;

        nativeBuildInputs = [rust pkgs.cargo-tarpaulin cargo-fuzz-wrapped pkgs.cargo-insta];
        RUST_PATH = "${rust}";
      };

      defaultPackage = naersk'.buildPackage ./.;
    });
}
