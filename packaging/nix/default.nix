# Nix build of shellcheck-rust from this checkout.
#   nix-build packaging/nix          # or: nix build -f packaging/nix
# Uses the workspace Cargo.lock, so no cargoHash is needed.
{ lib, rustPlatform }:

rustPlatform.buildRustPackage {
  pname = "shellcheck-rust";
  version = "0.0.1";

  src = lib.cleanSource ../..;
  cargoLock.lockFile = ../../Cargo.lock;

  cargoBuildFlags = [ "-p" "shellcheck-rust" ];
  cargoTestFlags = [ "-p" "shellcheck-rust" ];
  doCheck = false;

  meta = {
    description = "A Rust port of ShellCheck: finds bugs in shell scripts";
    homepage = "https://github.com/DevanshChoubey/shellcheck-rust";
    license = lib.licenses.gpl3Plus;
    mainProgram = "shellcheck-rust";
  };
}
