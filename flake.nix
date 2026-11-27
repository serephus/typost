{
  description = "typost — a Typst-powered static site generator";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crate2nix = {
      url = "github:nix-community/crate2nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixit = {
      url = "github:serephus/nixit";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
      inputs.rust-overlay.follows = "rust-overlay";
    };
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      rust-overlay,
      crate2nix,
      nixit,
      ...
    }:
    {
      # Declarative GitHub repository settings (managed with `nixit`).
      githubRepositories.typost = nixit.lib.githubRepository {
        owner = "serephus";
        name = "typost";
        description = "A Typst-powered static site generator.";
        homepage = "https://github.com/serephus/typost";
        topics = [
          "typst"
          "static-site-generator"
          "rust"
        ];
        visibility = "public";

        features = {
          wiki.enable = false;
          issues.enable = true;
          projects.enable = false;
          discussions.enable = false;
        };

        is_template = false;
        is_archived = false;

        pull = {
          merge.enable = true;
          squash.enable = false;
          rebase.enable = false;
          auto_merge = true;
          delete_branch_on_merge = true;
          update_branch = true;
        };

        actions = {
          enable = true;
          policy = "all";
          default_token_permissions = "read";
          allow_pr_approval = false;
        };

        rulesets = {
          default = {
            enforcement = "active";
            conditions.ref_name.include = [ "~DEFAULT_BRANCH" ];
            rules = [
              { type = "deletion"; }
              { type = "non_fast_forward"; }
            ];
          };
          pr = {
            enforcement = "active";
            conditions.ref_name.include = [ "~DEFAULT_BRANCH" ];
            rules = [
              {
                type = "required_status_checks";
                parameters = {
                  strict_required_status_checks_policy = true;
                  required_status_checks = [
                    { "context" = "ubuntu-latest-x86_64-unknown-linux-gnu-nightly"; }
                    { "context" = "ubuntu-latest-x86_64-unknown-linux-gnu-stable"; }
                    { "context" = "Nix Build"; }
                  ];
                };
              }
            ];
          };
        };
      };
    }
    // flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        rust = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rustfmt"
            "clippy"
            "rust-analyzer"
          ];
        };

        cargoNix = crate2nix.tools.${system}.appliedCargoNix {
          name = "typost";
          src = ./.;
        };

        # crate2nix builds the workspace crate-by-crate; the binary is `typost`.
        typost = (cargoNix.workspaceMembers.typost.build).overrideAttrs (old: {
          meta = (old.meta or { }) // {
            description = "A Typst-powered static site generator";
            mainProgram = "typost";
          };
        });

        # `nix flake check` compiles and runs the `typost` unit tests.
        tests = (cargoNix.workspaceMembers.typost.build).override {
          runTests = true;
        };
      in
      {
        packages.default = typost;
        packages.typost = typost;

        checks.default = tests;
        checks.tests = tests;

        apps.default = {
          type = "app";
          program = "${typost}/bin/typost";
          meta = {
            description = "typost — a Typst-powered static site generator";
          };
        };

        devShells.default = pkgs.mkShell {
          name = "typost";
          buildInputs = [
            rust
            pkgs.typst
            pkgs.tinymist
          ];
        };
      }
    );
}
