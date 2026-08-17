{
  lib,
  pkgs,
  version,
  src,
  clippy,
  rustfmt,
  cargo-llvm-cov,
  cargo-nextest,
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
      # prlimit: nextest wrapper script (.config/nextest.toml) caps test VA
      ++ lib.optionals (doTests || doCoverage) [pkgs.util-linux]
      ++ lib.optionals doCoverage [cargo-llvm-cov cargo-nextest];
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
        cargo llvm-cov report --text --output-path target/llvm-cov/coverage-report.txt
        mkdir -p "$out"
        cp -r target/llvm-cov/html/. "$out/"
        cp target/llvm-cov/coverage-report.txt "$out/"
      '';
    })
  else base
