# Wynd Contracts

## Adding a contract

To make a new contract.

```shell
cd contracts
cargo generate --git https://github.com/CosmWasm/cw-template.git --name wynd-NAME
cd wynd-NAME

rm -rf .github
rm .editorconfig .git* Cargo.lock Developing.md Importing.md LICENSE NOTICE Publishing.md rustfmt.toml

cargo fmt && cargo build && cargo test
git add .
```

Then go up, commit Cargo.lock and add a line to the CI file, copying like the other contracts

```shell
git add ../../Cargo.lock
```
