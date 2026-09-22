use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

pub use constants::*;
pub use error::*;
pub use instructions::*;
pub use state::*;

declare_id!("AcHkrwz4uVrpue7ja19ezxSNfZDKTUucRjw3yQd4oXW1");

#[program]
pub mod vault {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        instructions::vault::initialize(ctx)
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        instructions::vault::deposit(ctx, amount)
    }

    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
        instructions::vault::withdraw(ctx, amount)
    }

    pub fn close(ctx: Context<CloseVault>) -> Result<()> {
        instructions::vault::close(ctx)
    }

    pub fn make(
        ctx: Context<Make>,
        id: u64,
        amount_a: u64,
        amount_b: u64,
        taker: Pubkey,
        expires_at: i64,
    ) -> Result<()> {
        instructions::escrow::make(ctx, id, amount_a, amount_b, taker, expires_at)
    }

    pub fn update(
        ctx: Context<UpdateOffer>,
        amount_b: u64,
        taker: Pubkey,
        expires_at: i64,
    ) -> Result<()> {
        instructions::escrow::update(ctx, amount_b, taker, expires_at)
    }

    pub fn take(ctx: Context<Take>) -> Result<()> {
        instructions::escrow::take(ctx)
    }

    pub fn refund(ctx: Context<Refund>) -> Result<()> {
        instructions::escrow::refund(ctx)
    }
}
