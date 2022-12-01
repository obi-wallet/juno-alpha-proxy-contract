# CW1 Whitelist

DISCLAIMER: Obi and this contract release are in alpha. Security audits are pending. Current implementations are only intended for trial purposes.

This is a modification of CW1, a whitelist of addresses, originally
at https://github.com/CosmWasm/cw-plus/tree/main/contracts/cw1-whitelist.

The owner may `Execute` any message via the contract. Authorized addresses,
based on message contents or recurring spend limits, may also execute.

The immutability functionality of CW1 has been removed, and so
has the Freeze function.

In order to prevent the admin from accidentally updating to an account
that no one controls, the update process is now 2 steps:

1) ProposeUpdateAdmin {new_admin: String}, signed by current admin
2) ConfirmUpdateAdmin {}, signed by new admin

A delay can also be implemented so that CancelUpdateAdmin {} can
be called.

### Hot Wallets

Besides admins, the contract can accept "hot wallets" as defined
in hot_wallet.rs. These wallets can currently perform spend/transfer
cw20 or bank actions, as long as these don't go over the hot wallet's
set periodic spend limit.

### Fee Repayment

The contract can have a "fee debt," set upon instantiation. There may
be other ways for the contract to increase its debt in the future, as
long as admin is the signer. The contract attempts to repay this debt
whenever there is a coin send transaction of some kind.

Cw20 price support is currently spotty; therefore, cw20 transfers may
be rejected until the fee debt is repaid by sending USDC or native
asset.

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
