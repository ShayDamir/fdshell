{
  description = "FD Shell — security-oriented shell with fd passing.";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay, ... }: let
    inherit (nixpkgs) lib;
    eachSystem = lib.genAttrs lib.systems.flakeExposed;
    version = (builtins.fromTOML (builtins.readFile ./safe/fdshell/Cargo.toml)).package.version;
    pkgsFor = eachSystem (system:
      import nixpkgs {
        localSystem.system = system;
        overlays = [(import rust-overlay)];
      });
  in {
    packages = eachSystem (system: {
      default = pkgsFor.${system}.callPackage ./package.nix {
        inherit version;
        src = lib.cleanSource ./.;
        cargoLock = ./Cargo.lock;
      };
    });

    checks = eachSystem (system: {
      default = pkgsFor.${system}.callPackage ./package.nix {
        inherit version;
        src = lib.cleanSource ./.;
        cargoLock = ./Cargo.lock;
        doClippy = true;
        doTests = true;
      };
    });
  };
}
