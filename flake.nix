{
  description = "Rust development environment";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs = {
    self,
    nixpkgs,
  }: let
    systems = [
      "x86_64-linux"
      "aarch64-linux"
    ];
    forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
  in {
    devShells = forAllSystems (pkgs: {
      default = pkgs.mkShell {
        packages = with pkgs; [
          rustc
          cargo
          rust-analyzer
          clippy
          rustfmt
          pkg-config
        ];

        shellHook = ''
          export CARGO_HOME="$PWD/.direnv/cargo"
          export RUST_SRC_PATH="${pkgs.rustPlatform.rustLibSrc}"
          mkdir -p "$CARGO_HOME"
          echo "⚡ [Rust Dev Environment Loaded]"
          echo "  • Rust:  $(rustc --version 2>/dev/null || echo 'ready')"
          echo "  • Cargo: $(cargo --version 2>/dev/null || echo 'ready')"
          echo "  • Cargo home: $CARGO_HOME"
        '';
      };
    });
  };
}
