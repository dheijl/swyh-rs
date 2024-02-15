{
  description = ''
    Stream What You Hear written in rust, inspired by SWYH
  '';
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs = {
      nixpkgs.follows = "nixpkgs";
      flake-utils.follows = "flake-utils";
    };
    nixpkgs.url = github:NixOS/nixpkgs/nixos-23.11;
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    rust-overlay,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        rust = pkgs.rust-bin.stable.latest.default;
        swyh-rs =
          pkgs.callPackage ./default.nix {};
      in {
        packages = {
          inherit swyh-rs;
          swyh-rs-cli = swyh-rs.override {withGui = false;};
          swyh-rs-gui = swyh-rs.override {withCli = false;};
          default = swyh-rs;
        };
        devShells = {
          swyh-rs = swyh-rs.devShell;
          default = swyh-rs.devShell;
        };
        formatter = pkgs.alejandra;
      }
    );
}
