{ lib, pkgs, version, src, cargoLock, clippy, rustfmt
, rustToolchainFile
, cargo-llvm-cov ? null, cargo-nextest ? null
, doClippy ? false, doTests ? false, doFmt ? false, doCoverage ? false
}:

let
  toolchain = pkgs.rust-bin.fromRustupToolchainFile rustToolchainFile;

  rustPlatform' = pkgs.makeRustPlatform {
    rustc = toolchain;
    cargo = toolchain;
  };

  base = rustPlatform'.buildRustPackage {
    pname = "fdshell";
    inherit version src;
    cargoLock.lockFile = cargoLock;
    meta.mainProgram = "fdshell";

    useNextest = doTests || doCoverage;
    dontCargoCheck = !doTests && !doClippy && !doFmt && !doCoverage;
    cargoTestFlags = lib.optionals doTests [ ];
    nativeBuildInputs = lib.optionals doClippy [ clippy ]
      ++ lib.optionals doFmt [ rustfmt ]
      ++ lib.optionals doCoverage [ cargo-llvm-cov cargo-nextest ];
    preCheck = lib.optionalString doFmt ''
      cargo fmt --check
    '' + lib.optionalString doClippy ''
      cargo clippy --all-targets -- -D warnings
    '';
  };
in
if doCoverage then base.overrideAttrs (old: {
  dontCargoCheck = true;
  checkPhase = ''
    cargo llvm-cov nextest --html
    cp -r target/llvm-cov/html $out/
  '';
}) else base
