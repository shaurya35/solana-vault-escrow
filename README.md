# Solana Vault and Escrow

An Anchor program that implements a secure SOL vault, an atomic SPL-token escrow, a clock-gated timed escrow, and an optional in-kind redemption vault. The integration suite runs entirely in-process with LiteSVM.

![LiteSVM test results](docs/test-results.png)

## Assignment coverage

### 1. SOL vault

- Deterministic state and vault PDAs per owner.
- Deposit any positive amount of SOL.
- Owner-only withdrawals with balance checks.
- Owner-only close that returns all SOL and state-account rent.
- Tests cover zero deposits, excessive withdrawals, unauthorized withdrawals, and closure.

### 2. SPL-token escrow

- `make`: creates an offer PDA and deposits token A into a program-controlled token vault.
- `update`: lets only the maker change the requested token-B amount, designated taker, and deadline.
- `take`: atomically transfers token B to the maker and token A to the taker, then closes the vault and offer.
- `refund`: returns token A and closes the escrow.
- Typed token accounts, mint relationships, signer checks, PDA constraints, balance checks, and checked token transfers protect every path.

### 3. LiteSVM tests

Rust integration tests cover every required instruction plus failure paths:

- SOL vault initialize, deposit, withdraw, attacker rejection, insufficient funds, zero amount, and close.
- Escrow make, update, designated-taker enforcement, atomic take, and account closure.
- Timed escrow early-refund rejection, expired-take rejection, and post-deadline refund.
- In-kind pool initialization, proportional deposit, unbalanced-deposit rejection, unauthorized redemption rejection, partial redemption, and full exit.

### 4. Timed escrow extension

Each offer stores an optional `expires_at` Unix timestamp from the Clock sysvar:

- `expires_at = 0` creates an offer without a deadline and allows maker refund at any time.
- Before or at the deadline, the designated taker may settle the offer.
- Before the deadline, the maker cannot refund a timed offer.
- After the deadline, taking and updating are disabled and the maker can refund.

### 5. In-kind redemption extension

The optional two-asset pool issues fungible share tokens. A holder can burn shares at any time and receive the same pro-rata percentage of both underlying token reserves. Redemption therefore does not depend on selling a position into a quote asset or on maintaining separate withdrawal liquidity.

This is **non-custodial in the economic/enforcement sense**: the program cannot arbitrarily deny a valid share holder's proportional exit, and no administrator signs redemptions. The assets are still held by program-derived token accounts, so it is not literal wallet self-custody. The demonstration intentionally supports two standard SPL mints with equal decimals and an initial 1:1 raw-unit deposit; a production portfolio vault would need oracle-aware valuation, slippage bounds, governance, emergency handling, and an audited share-pricing model.

## Architecture

```text
programs/solana-vault-escrow/
├── src/
│   ├── lib.rs                  # Anchor entrypoints
│   ├── constants.rs            # PDA seeds
│   ├── error.rs                # Shared custom errors
│   ├── state.rs                # Vault, offer, and in-kind pool state
│   └── instructions/
│       ├── vault.rs            # SOL vault logic
│       ├── escrow.rs           # SPL and timed escrow logic
│       └── in_kind.rs          # Share minting and pro-rata redemption
└── tests/
    ├── common/mod.rs           # LiteSVM and SPL-token test helpers
    ├── vault.rs
    ├── escrow.rs
    └── in_kind.rs
```

## Security properties

- Anchor `Signer`, typed account, `has_one`, address, mint, authority, seed, and bump constraints validate all caller-controlled accounts.
- Vault and escrow assets move only through PDA-signed CPIs to the System or SPL Token program.
- `transfer_checked` binds transfers to the expected mint and decimals.
- Escrow payment and delivery execute in one Solana transaction, so any later failure rolls back the earlier transfer.
- Checked arithmetic and proportionality checks protect in-kind share accounting.
- Closing returns rent to the expected owner/maker and removes completed state.

## Build and test

Prerequisites: Rust 1.89, Solana CLI, and Anchor CLI compatible with Anchor 1.2.

```bash
NO_DNA=1 anchor build
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --nocapture
```

The tests load `target/deploy/solana_vault_escrow.so`, so run `anchor build` before `cargo test` whenever the on-chain program changes.

## Program ID

Local program ID: `AcHkrwz4uVrpue7ja19ezxSNfZDKTUucRjw3yQd4oXW1`

## License

[MIT](LICENSE)
