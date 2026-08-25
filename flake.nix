{
  description = "Nix flake for Rust Rogue";

  inputs = {
    # Stable channel for the latest packages (including Rust toolchain).
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

    # Utility to generate outputs for every supported system.
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        devShells.default = pkgs.mkShell {
          packages =
            (with pkgs; [
              # ---- Rust ----
              # Rust toolchain from nixpkgs
              rustc
              cargo
              rust-analyzer # LSP server
              rustfmt # code formatter
              clippy # linting

              # Common cargo subcommands. Add or remove as needed.
              # cargo-edit # cargo add/remove/upgrade
              # cargo-nextest # fast test runner
              # cargo-watch # watch files and re-run commands
              # cargo-audit # security audit of dependencies

              # ---- C / C++ ----
              gcc # compiler
              gnumake # build automation
              autoconf # Autotools
              automake # Autotools
              libtool # Autotools
              pkg-config # library compiler flags discovery
              gdb # debugger

              # ── Add C/C++ libraries HERE ──────────────────────────────
              # Libraries from nixpkgs ship with headers (.h) and are found
              # automatically by pkg-config / make. Uncomment examples:
              # zlib        # compression library
              # openssl     # TLS/crypto (dev headers provided)
              # sqlite      # embedded SQL database
              # libcurl     # HTTP client
              # readline    # line editing
              ncurses
              # ───────────────────────────────────────────────────────────
            ])

            # binutils is needed for cross-compilation on Linux, but not on macOS.
            ++ pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.binutils ];
          # Optional LSP support for C/C++ editors. Uncomment to enable.
          # ++ [ pkgs.clang-tools ]; # provides clangd + clang-format

          shellHook = ''
            # ---- Rust ----
            # Point rust-analyzer at the toolchain sources
            export RUST_SRC_PATH="$(rustc --print sysroot)/lib/rustlib/src/rust/library"
            # Always print backtraces on panic
            export RUST_BACKTRACE=1

            # ---- C / C++ ----
            export CC=gcc
            export CXX=g++
            # Use all cores for parallel builds
            export MAKEFLAGS="-j$(nproc)"

            echo "🦀⚙️  Rust + C development environment ready"
            rustc --version
            cargo --version
            gcc --version | head -n 1
            make --version | head -n 1
          '';
        };
      }
    );
}
