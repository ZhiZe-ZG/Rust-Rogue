{
  description = "Nix flake for Rogue 5.4.4";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      forAllSystems =
        f:
        nixpkgs.lib.genAttrs systems (
          system:
          f {
            pkgs = import nixpkgs { inherit system; };
          }
        );
    in
    {
      packages = forAllSystems (
        { pkgs }:
        let
          rogue = pkgs.stdenv.mkDerivation {
            pname = "rogue";
            version = "5.4.4";
            src = self;

            nativeBuildInputs = [
              pkgs.pkg-config
              pkgs.cargo
              pkgs.rustc
            ];

            buildInputs = [
              pkgs.ncurses
            ];

            configureFlags = [
              "--with-program-name=rogue"
            ];

            installPhase = ''
              runHook preInstall

              install -Dm755 rogue $out/bin/rogue

              if [ -f rogue.6 ]; then
                install -Dm644 rogue.6 $out/share/man/man6/rogue.6
              fi

              runHook postInstall
            '';

            meta = with pkgs.lib; {
              description = "Classic Rogue dungeon crawler";
              homepage = "https://github.com/rogueforge/rogue";
              license = licenses.bsd3;
              platforms = platforms.unix;
              mainProgram = "rogue";
            };
          };
        in
        {
          default = rogue;
          rogue = rogue;
        }
      );

      devShells = forAllSystems (
        { pkgs }: {
          default = pkgs.mkShell {
            packages = with pkgs; [
              autoconf
              automake
              cargo
              gnumake
              ncurses
              pkg-config
              rustc
              rust-analyzer
            ];
          };
        }
      );
    };
}
