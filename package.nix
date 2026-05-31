{
  lib,
  pkgs,
  version,
  src,
  clippy,
  rustfmt,
  cargo-llvm-cov,
  cargo-nextest,
  jq,
  doClippy ? false,
  doTests ? false,
  doFmt ? false,
  doCoverage ? false,
}: let
  toolchain = pkgs.rust-bin.fromRustupToolchainFile (src + "/rust-toolchain.toml");

  rustPlatform' = pkgs.makeRustPlatform {
    rustc = toolchain;
    cargo = toolchain;
  };

  base = rustPlatform'.buildRustPackage {
    pname = "fdshell";
    inherit version src;
    cargoLock.lockFile = src + "/Cargo.lock";
    meta.mainProgram = "fdshell";

    useNextest = doTests || doCoverage;
    dontCargoCheck = !doTests && !doClippy && !doFmt && !doCoverage;
    cargoTestFlags = lib.optionals doTests [];
    nativeBuildInputs =
      lib.optionals doClippy [clippy]
      ++ lib.optionals doFmt [rustfmt]
      ++ lib.optionals doCoverage [cargo-llvm-cov cargo-nextest jq];
    preCheck =
      lib.optionalString doFmt ''
        cargo fmt --check
      ''
      + lib.optionalString doClippy ''
        cargo clippy --all-targets -- -D warnings
      '';
  };
in
  if doCoverage
  then
    base.overrideAttrs (old: {
      dontCargoCheck = true;
      checkPhase = ''
        cargo llvm-cov nextest --html
        cargo llvm-cov report --json --summary-only --output-path target/llvm-cov/coverage-summary.json
        cp -r target/llvm-cov/html $out/
        cp target/llvm-cov/coverage-summary.json $out/
      '';
    })
  else base
