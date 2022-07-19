# CW1 Whitelist

This is a modification of CW1, a whitelist of addresses, originally
at https://github.com/CosmWasm/cw-plus/tree/main/contracts/cw1-whitelist.

1 single admin may `Execute` any message via the contract. This
is intended to be used with a native multisig in order to achieve
an "updatable multisig" for single user multisigs without the
propose+vote+execute transaction overhead of cw3.

The immutability functionality of CW1 has been removed, and so
has the Freeze function.

In order to prevent the admin from accidentally updating to an account
that no one controls, the update process is now 2 steps:

1) ProposeUpdateAdmin {new_admin: String}, signed by current admin
2) ConfirmUpdateAdmin {}, signed by new admin

## Allowing Custom Messages

By default, this doesn't support `CustomMsg` in order to be fully generic
among blockchains. However, all types are Generic over `T`, and this is only
fixed in `handle`. You can import this contract and just redefine your `handle`
function, setting a different parameter to `ExecuteMsg`, and you can produce
a chain-specific message.

## Running this contract

You will need Rust 1.44.1+ with `wasm32-unknown-unknown` target installed.

You can run unit tests on this via: 

`cargo test`

Once you are happy with the content, you can compile it to wasm via:

```
RUSTFLAGS='-C link-arg=-s' cargo wasm
cp ../../target/wasm32-unknown-unknown/release/cw1_whitelist.wasm .
ls -l cw1_whitelist.wasm
sha256sum cw1_whitelist.wasm
```

Or for a production-ready (optimized) build, run a build command in the
the repository root: https://github.com/CosmWasm/cw-plus#compiling.
