{
  description = "Nix flake for Rogue 5.4.4";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
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
