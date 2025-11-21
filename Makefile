options.md: nix/module.nix flake.nix fmt
	rm -f ./options.md || true
	cp $$(nix build .#doc --no-link --print-out-paths) ./options.md
	chmod 755 ./options.md
	git add ./options.md nix/key_codes.nix nix/axis_codes.nix nix/bus_types.nix
	git commit -m 'Update options.md'

.PHONY: nix/key_codes.nix nix/axis_codes.nix nix/bus_types.nix fmt

nix/key_codes.nix:
	cargo run --bin tweakpoint -- --list-keys | sed -r -e '1 i [' -e '$$ a ]' -e 's/^|$$/"/g' > nix/key_codes.nix

nix/axis_codes.nix:
	cargo run --bin tweakpoint -- --list-relative-axes | sed -r -e '1 i [' -e '$$ a ]' -e 's/^|$$/"/g' > nix/axis_codes.nix

nix/bus_types.nix:
	cargo run --bin tweakpoint -- --list-bus-types | sed -r -e '1 i [' -e '$$ a ]' -e 's/^|$$/"/g' > nix/bus_types.nix

fmt: nix/key_codes.nix nix/axis_codes.nix nix/bus_types.nix
	nixfmt *.nix nix/*.nix
