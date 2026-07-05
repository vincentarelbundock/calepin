{
  description = "Calepin - Computational Notebooks and Static Websites in typst.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "calepin";
          version = "0.0.30";

          src = ./.;

          cargoLock.lockFile = ./Cargo.lock;
          buildAndTestSubdir = "calepin";

          nativeBuildInputs = [ pkgs.makeWrapper ];

          postFixup = ''
            wrapProgram $out/bin/calepin \
              --suffix PATH : ${pkgs.lib.makeBinPath [ pkgs.typst ]}
          '';

          meta = with pkgs.lib; {
            description = "A Rust CLI for preprocessing Typst documents with executable code chunks";
            homepage = "https://vincentarelbundock.github.io/calepin";
            license = licenses.mit;
            mainProgram = "calepin";
          };
        };

        devShells.default = pkgs.mkShell {
          shellHook = ''
            export PATH="$PWD/target/debug:$PATH"
            export LD_LIBRARY_PATH="${pkgs.stdenv.cc.cc.lib}/lib:$LD_LIBRARY_PATH"
          '';
          packages = with pkgs; [
            # Rust
            cargo
            rustc
            clippy
            rust-analyzer
            rustfmt
            # Node
            nodejs
            # Python
            uv
            # R for docs
            (rWrapper.override {
              packages = with rPackages; [ ggplot2 tinytable ];
            })
            # Other
            typst
          ];
        };
      });
}
