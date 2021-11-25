<p align="center">
  <a href="https://wyndex.io/">
    <img alt="WYND" src="http://i.epvpimg.com/fjQacab.png" width="250" />
  </a>
</p>
<h1 align="center">
  Contracts
</h1>


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

## Building all wasm contracts

On the top level dir:

```json
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/workspace-optimizer:0.12.3
```

Look inside the `artifacts` dir that will be created.
