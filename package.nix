{ lib, rustPlatform, version, src, cargoLock, clippy
, doClippy ? false, doTests ? false }:

rustPlatform.buildRustPackage {
  pname = "fdshell";
  inherit version src;
  cargoLock.lockFile = cargoLock;
  meta.mainProgram = "fdshell";

  useNextest = doTests;
  dontCargoCheck = !doTests && !doClippy;
  cargoTestFlags = lib.optionals doTests [ "--no-tests" "pass" ];
  nativeBuildInputs = lib.optionals doClippy [ clippy ];
  preCheck = lib.optionalString doClippy ''
    cargo clippy --all-targets -- -D warnings
  '';
}
