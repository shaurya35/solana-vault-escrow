use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct VaultState {
    pub owner: Pubkey,
    pub state_bump: u8,
    pub vault_bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Offer {
    pub maker: Pubkey,
    pub taker: Pubkey,
    pub mint_a: Pubkey,
    pub mint_b: Pubkey,
    pub id: u64,
    pub amount_a: u64,
    pub amount_b: u64,
    pub expires_at: i64,
    pub offer_bump: u8,
    pub vault_bump: u8,
}

/// A two-asset vault whose fungible shares are always redeemable pro rata.
///
/// Redemption transfers the underlying assets themselves instead of requiring
/// the vault to sell them for a liquid settlement asset first.
#[account]
#[derive(InitSpace)]
pub struct InKindPool {
    pub authority: Pubkey,
    pub asset_a_mint: Pubkey,
    pub asset_b_mint: Pubkey,
    pub share_mint: Pubkey,
    pub bump: u8,
}
