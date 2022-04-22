# blackjack-terra

Simple blackjack game built on terra using cosmwasm.

TBU

## build

### build contracts

```sh
cargo wasm
```

### build contracts (optimizer)

```
OPTIMIZER_VERSION="0.12.6"

alias workspace-optimizer='docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/workspace-optimizer:${OPTIMIZER_VERSION}'

workspace-optimizer
```

### update schema

```sh
./update_schema.sh
```

### deploy

see ref



## test

```sh
cargo unit-test
```