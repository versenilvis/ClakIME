{
  description = "Fast and highly stable Vietnamese input method for Fcitx5 and Wayland";

  inputs = {
    systems.url = "github:nix-systems/default";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
  };

  outputs = inputs @ {flake-parts, ...}:
    flake-parts.lib.mkFlake
    {
      inherit inputs;
    }
    (
      {
        inputs,
        self,
        ...
      }: {
        systems = import inputs.systems;
        perSystem = {
          pkgs,
          self',
          ...
        }: {
          packages = {
            clak = pkgs.stdenv.mkDerivation rec {
              pname = "clak";
              version = self.shortRev or "0.1.0";
              src = pkgs.lib.cleanSource ./.;

              cargoDeps = pkgs.rustPlatform.importCargoLock {
                lockFile = ./engine/Cargo.lock;
              };
              cargoRoot = "engine";

              nativeBuildInputs = with pkgs; [
                cmake
                kdePackages.extra-cmake-modules
                rustPlatform.cargoSetupHook
                rustc
                cargo
                pkg-config
              ];

              buildInputs = with pkgs; [
                fcitx5
                libinput
                systemd
              ];

              cmakeFlags = [
                "-DCMAKE_BUILD_TYPE=Release"
                "-DCMAKE_INSTALL_PREFIX=${placeholder "out"}"
                "-DENABLE_GUI=OFF"
                "-DENABLE_CLI=OFF"
                "-DBUILD_TESTING=OFF"
              ];

              meta = with pkgs.lib; {
                description = "Fast and highly stable Vietnamese input method for Fcitx5 and Wayland";
                homepage = "https://github.com/versenilvis/clak";
                license = licenses.bsd0;
                platforms = platforms.linux;
              };
            };
            default = self'.packages.clak;
          };

          devShells.default = pkgs.mkShell {
            packages = with pkgs; [
              cmake
              kdePackages.extra-cmake-modules
              fcitx5
              rustc
              cargo
              just
              git-cliff
            ];
          };
        };
      }
    );
}
