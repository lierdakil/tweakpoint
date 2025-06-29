{
  description = "Tweakpoint -- a small daemon tweaking pointer device behaviour";

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      ...
    }:
    let
      mkPackage =
        pkgs:
        let
          manifest = (pkgs.lib.importTOML ./Cargo.toml).package;
        in
        pkgs.rustPlatform.buildRustPackage {
          pname = manifest.name;
          version = manifest.version;
          src = pkgs.lib.cleanSource (
            pkgs.lib.sources.sourceFilesBySuffices ./. [
              "Cargo.lock"
              "Cargo.toml"
              ".rs"
            ]
          );
          cargoLock.lockFile = ./Cargo.lock;
        };
    in
    {
      homeManagerModules.default = import ./nix/module.nix self;
      overlays.default = prev: final: {
        tweakpoint = mkPackage final;
      };
    }
    //
      flake-utils.lib.eachSystem
        [ flake-utils.lib.system.x86_64-linux flake-utils.lib.system.aarch64-linux ]
        (
          system:
          let
            pkgs = nixpkgs.legacyPackages.${system};
          in
          {
            devShells.default = pkgs.mkShell {
              buildInputs = with pkgs; [
                rustc
                cargo
                clippy
                rust-analyzer
                rustfmt
              ];
              # Environment variables
              RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
            };
            packages.default = mkPackage pkgs;
            packages.static =
              (import nixpkgs {
                inherit system;
                overlays = [ self.overlays.default ];
              }).pkgsStatic.tweakpoint;
            packages.doc =
              let
                eval = pkgs.lib.evalModules {
                  modules = [
                    {
                      options._module.args = pkgs.lib.mkOption { internal = true; };
                      config._module.args = { inherit pkgs; };
                      config._module.check = false;
                    }
                    self.homeManagerModules.default
                  ];
                };
              in
              (pkgs.nixosOptionsDoc { inherit (eval) options; }).optionsCommonMark;
            apps.default = flake-utils.lib.mkApp {
              drv = self.packages.${system}.default;
            };
            checks.config =
              let
                eval = pkgs.lib.evalModules {
                  modules = [
                    {
                      options._module.args = pkgs.lib.mkOption { internal = true; };
                      config._module.args = { inherit pkgs; };
                      config._module.check = false;
                      config.services.tweakpoint = {
                        enable = true;
                        settings = {
                          device = "/dev/input/by-id/usb-Foo-Bar";
                          meta.key = "BTN_SIDE";
                          btn_map = {
                            BTN_LEFT = "BTN_RIGHT";
                            BTN_RIGHT = "BTN_LEFT";
                          };
                          meta.chord.BTN_LEFT = "ToggleScroll";
                          meta.chord.BTN_RIGHT.ToggleLock = [
                            "BTN_LEFT"
                            "BTN_RIGHT"
                          ];
                          meta.chord.BTN_MIDDLE.Button = "BTN_SIDE";
                          axis_map.regular = {
                            "REL_X" = {
                              axis = "REL_Y";
                              factor = 0.1;
                            };
                          };
                          axis_map.scroll = {
                            "REL_X".axis = "REL_Y";
                          };
                        };
                      };
                    }
                    self.homeManagerModules.default
                  ];
                };
              in
              eval.config.services.tweakpoint.settingsFile;
          }
        );
}
