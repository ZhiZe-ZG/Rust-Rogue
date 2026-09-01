{
  description = "Nix flake for Rust Rogue";

  inputs = {
    # Stable channel for the latest packages.
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

    # Utility to generate outputs for every supported system.
    flake-utils.url = "github:numtide/flake-utils";

    # Rust toolchain (rustc, cargo, rustfmt, clippy, rust-src, ...).
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        # Rust toolchain (stable, 1.98.0) with rust-src and rust-analyzer.
        rust-toolchain = pkgs.rust-bin.stable."1.98.0".default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "rogue";
          version = "5.4.4";

          src = self;

          buildAndTestSubdir = "src/rust";
          cargoLock.lockFile = ./src/rust/Cargo.lock;

          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = [ pkgs.ncurses ];

          meta = {
            description = "Rogue: Exploring the Dungeons of Doom (Rust port)";
            mainProgram = "rogue";
          };
        };

        devShells.default = pkgs.mkShell {
          packages =
            (with pkgs; [
              # ---- Rust ----
              rust-toolchain

              # Common cargo subcommands. Add or remove as needed.
              # cargo-edit # cargo add/remove/upgrade
              # cargo-nextest # fast test runner
              # cargo-watch # watch files and re-run commands
              # cargo-audit # security audit of dependencies

              # ---- Libraries ----
              pkg-config # library compiler flags discovery
              ncurses # rust ncurses bindings still need the ncurses library
            ])
            ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [ pkgs.gdb ];

          shellHook = ''
            # ---- Rust ----
            # Point rust-analyzer at the toolchain sources
            export RUST_SRC_PATH="${rust-toolchain}/lib/rustlib/src/rust/library"
            # Always print backtraces on panic
            export RUST_BACKTRACE=1

            echo "🦀 Rust development environment ready"
            rustc --version
            cargo --version
          '';
        };
      }
    );
}