{
  description = "MatrixShot — Wayland screenshot + recording suite";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in {
      packages.${system}.default = pkgs.rustPlatform.buildRustPackage {
        pname = "matrixshot";
        version = "0.1.0";
        src = ./.;
        cargoLock.lockFile = ./Cargo.lock;
        meta = with pkgs.lib; {
          description = "Wayland screenshot and screen-recording suite";
          license = licenses.mit;
          mainProgram = "matrixshot";
          platforms = platforms.linux;
        };
      };
      apps.${system}.default = {
        type = "app";
        program = "${self.packages.${system}.default}/bin/matrixshot";
      };
      devShells.${system}.default = pkgs.mkShell {
        packages = with pkgs; [ rustc cargo grim slurp wl-clipboard gpu-screen-recorder imv ];
      };
      checks.${system}.matrixshot = self.packages.${system}.default;
    };
}
