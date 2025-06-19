{
  description = "Tweakpoint -- a small daemon tweaking pointer device behaviour";

  outputs = { self, nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachSystem [flake-utils.lib.system.x86_64-linux flake-utils.lib.system.aarch64-linux] (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        manifest = (pkgs.lib.importTOML ./Cargo.toml).package;
      in
      with pkgs;
      {
        inherit pkgs;
        devShells.default = mkShell {
          buildInputs = [
            rustc
            cargo
            clippy
            rust-analyzer
            rustfmt
          ];
          # Environment variables
          RUST_SRC_PATH = rustPlatform.rustLibSrc;
        };
        packages.default = rustPlatform.buildRustPackage {
          pname = manifest.name;
          version = manifest.version;
          src = lib.cleanSource ./.;
          cargoLock.lockFile = ./Cargo.lock;
        };
        apps.default = flake-utils.lib.mkApp {
          drv = self.packages.${system}.default;
        };
      }
    );
}
