{ lib, rustPlatform, version, src, cargoLock, clippy, rustfmt
, doClippy ? false, doTests ? false, doFmt ? false }:

rustPlatform.buildRustPackage {
  pname = "fdshell";
  inherit version src;
  cargoLock.lockFile = cargoLock;
  meta.mainProgram = "fdshell";

  useNextest = doTests;
  dontCargoCheck = !doTests && !doClippy && !doFmt;
  cargoTestFlags = lib.optionals doTests [ ];
  nativeBuildInputs = lib.optionals doClippy [ clippy ]
    ++ lib.optionals doFmt [ rustfmt ];
  preCheck = lib.optionalString doFmt ''
    cargo fmt --check
  '' + lib.optionalString doClippy ''
    cargo clippy --all-targets -- -D warnings
  '';
}
