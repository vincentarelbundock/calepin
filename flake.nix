{
  description = "Calepin - computational notebooks and static websites in Typst";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        lib = pkgs.lib;

        cargoToml = builtins.fromTOML (builtins.readFile ./calepin/Cargo.toml);
        rustSourceFilter = path: type:
          let
            rel = lib.removePrefix "${toString ./.}/" (toString path);
          in
          rel == "Cargo.toml"
          || rel == "Cargo.lock"
          || rel == "LICENSE"
          || rel == "README.md"
          || rel == "calepin"
          || lib.hasPrefix "calepin/" rel;

        calepin = pkgs.rustPlatform.buildRustPackage {
          pname = cargoToml.package.name;
          version = cargoToml.package.version;

          src = lib.cleanSourceWith {
            src = ./.;
            filter = rustSourceFilter;
          };

          cargoLock.lockFile = ./Cargo.lock;
          buildAndTestSubdir = "calepin";

          nativeBuildInputs = [ pkgs.makeWrapper ];

          postFixup = ''
            wrapProgram $out/bin/calepin \
              --suffix PATH : ${lib.makeBinPath [ pkgs.typst ]}
          '';

          meta = with lib; {
            description = "A Rust CLI for preprocessing Typst documents with executable code chunks";
            homepage = "https://vincentarelbundock.github.io/calepin";
            license = licenses.mit;
            mainProgram = "calepin";
          };
        };

        calepinApp = {
          type = "app";
          program = lib.getExe calepin;
          meta.description = calepin.meta.description;
        };

        rForWebsite = pkgs.rWrapper.override {
          packages = with pkgs.rPackages; [
            ggplot2
            tinytable
          ];
        };

        websiteTools = with pkgs; [
          cargo
          clippy
          d2
          graphviz
          mermaid-cli
          nodejs
          pdf2svg
          rust-analyzer
          rustc
          rustfmt
          tectonic
          texlivePackages.dvisvgm
          typst
          uv
          rForWebsite
        ];
      in
      {
        packages = {
          calepin = calepin;
          default = calepin;
        };

        apps = {
          calepin = calepinApp;
          default = calepinApp;
        };

        devShells = {
          default = pkgs.mkShell {
            packages = [
              calepin
              pkgs.typst
            ];
          };

          website = pkgs.mkShell {
            packages = websiteTools;

            shellHook = ''
              export PATH="$PWD/target/debug:$PATH"
              export LD_LIBRARY_PATH="${pkgs.stdenv.cc.cc.lib}/lib:$LD_LIBRARY_PATH"
            '';
          };
        };
      });
}
