use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, MintTo, Token, TokenAccount, TransferChecked};

use crate::{
    InKindPool, VaultEscrowError, ASSET_A_VAULT_SEED, ASSET_B_VAULT_SEED, IN_KIND_POOL_SEED,
    SHARE_MINT_SEED,
};

pub(crate) fn initialize(ctx: Context<InitializeInKindPool>) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    pool.authority = ctx.accounts.authority.key();
    pool.asset_a_mint = ctx.accounts.asset_a_mint.key();
    pool.asset_b_mint = ctx.accounts.asset_b_mint.key();
    pool.share_mint = ctx.accounts.share_mint.key();
    pool.bump = ctx.bumps.pool;
    Ok(())
}

pub(crate) fn deposit(ctx: Context<DepositInKind>, amount_a: u64, amount_b: u64) -> Result<()> {
    require!(amount_a > 0 && amount_b > 0, VaultEscrowError::ZeroAmount);

    let supply = ctx.accounts.share_mint.supply;
    let reserve_a = ctx.accounts.asset_a_vault.amount;
    let reserve_b = ctx.accounts.asset_b_vault.amount;

    let shares = if supply == 0 {
        require!(amount_a == amount_b, VaultEscrowError::UnbalancedDeposit);
        amount_a
    } else {
        let left = u128::from(amount_a)
            .checked_mul(u128::from(reserve_b))
            .ok_or(VaultEscrowError::MathOverflow)?;
        let right = u128::from(amount_b)
            .checked_mul(u128::from(reserve_a))
            .ok_or(VaultEscrowError::MathOverflow)?;
        require!(left == right, VaultEscrowError::UnbalancedDeposit);

        amount_a
            .checked_mul(supply)
            .ok_or(VaultEscrowError::MathOverflow)?
            .checked_div(reserve_a)
            .ok_or(VaultEscrowError::MathOverflow)?
    };

    require!(shares > 0, VaultEscrowError::RedemptionTooSmall);

    transfer_user_asset(
        &ctx.accounts.user_asset_a,
        &ctx.accounts.asset_a_mint,
        &ctx.accounts.asset_a_vault,
        &ctx.accounts.user,
        &ctx.accounts.token_program,
        amount_a,
    )?;
    transfer_user_asset(
        &ctx.accounts.user_asset_b,
        &ctx.accounts.asset_b_mint,
        &ctx.accounts.asset_b_vault,
        &ctx.accounts.user,
        &ctx.accounts.token_program,
        amount_b,
    )?;

    let authority_key = ctx.accounts.pool.authority;
    let asset_a_key = ctx.accounts.pool.asset_a_mint;
    let asset_b_key = ctx.accounts.pool.asset_b_mint;
    let bump = [ctx.accounts.pool.bump];
    let pool_seeds: &[&[u8]] = &[
        IN_KIND_POOL_SEED,
        authority_key.as_ref(),
        asset_a_key.as_ref(),
        asset_b_key.as_ref(),
        &bump,
    ];
    let signer = &[pool_seeds];

    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            MintTo {
                mint: ctx.accounts.share_mint.to_account_info(),
                to: ctx.accounts.user_shares.to_account_info(),
                authority: ctx.accounts.pool.to_account_info(),
            },
            signer,
        ),
        shares,
    )
}

pub(crate) fn redeem(ctx: Context<RedeemInKind>, shares: u64) -> Result<()> {
    require!(shares > 0, VaultEscrowError::ZeroAmount);

    let supply = ctx.accounts.share_mint.supply;
    require!(supply > 0, VaultEscrowError::EmptyShareSupply);

    let amount_a = pro_rata(ctx.accounts.asset_a_vault.amount, shares, supply)?;
    let amount_b = pro_rata(ctx.accounts.asset_b_vault.amount, shares, supply)?;
    require!(
        amount_a > 0 || amount_b > 0,
        VaultEscrowError::RedemptionTooSmall
    );

    token::burn(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            Burn {
                mint: ctx.accounts.share_mint.to_account_info(),
                from: ctx.accounts.user_shares.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        shares,
    )?;

    let authority_key = ctx.accounts.pool.authority;
    let asset_a_key = ctx.accounts.pool.asset_a_mint;
    let asset_b_key = ctx.accounts.pool.asset_b_mint;
    let bump = [ctx.accounts.pool.bump];
    let pool_seeds: &[&[u8]] = &[
        IN_KIND_POOL_SEED,
        authority_key.as_ref(),
        asset_a_key.as_ref(),
        asset_b_key.as_ref(),
        &bump,
    ];
    let signer = &[pool_seeds];

    transfer_pool_asset(
        &ctx.accounts.asset_a_vault,
        &ctx.accounts.asset_a_mint,
        &ctx.accounts.user_asset_a,
        &ctx.accounts.pool,
        &ctx.accounts.token_program,
        signer,
        amount_a,
    )?;
    transfer_pool_asset(
        &ctx.accounts.asset_b_vault,
        &ctx.accounts.asset_b_mint,
        &ctx.accounts.user_asset_b,
        &ctx.accounts.pool,
        &ctx.accounts.token_program,
        signer,
        amount_b,
    )
}

fn pro_rata(reserve: u64, shares: u64, supply: u64) -> Result<u64> {
    let value = u128::from(reserve)
        .checked_mul(u128::from(shares))
        .ok_or(VaultEscrowError::MathOverflow)?
        .checked_div(u128::from(supply))
        .ok_or(VaultEscrowError::MathOverflow)?;
    u64::try_from(value).map_err(|_| VaultEscrowError::MathOverflow.into())
}

fn transfer_user_asset<'info>(
    from: &Account<'info, TokenAccount>,
    mint: &Account<'info, Mint>,
    to: &Account<'info, TokenAccount>,
    user: &Signer<'info>,
    token_program: &Program<'info, Token>,
    amount: u64,
) -> Result<()> {
    token::transfer_checked(
        CpiContext::new(
            token_program.key(),
            TransferChecked {
                from: from.to_account_info(),
                mint: mint.to_account_info(),
                to: to.to_account_info(),
                authority: user.to_account_info(),
            },
        ),
        amount,
        mint.decimals,
    )
}

#[allow(clippy::too_many_arguments)]
fn transfer_pool_asset<'info>(
    from: &Account<'info, TokenAccount>,
    mint: &Account<'info, Mint>,
    to: &Account<'info, TokenAccount>,
    pool: &Account<'info, InKindPool>,
    token_program: &Program<'info, Token>,
    signer: &[&[&[u8]]],
    amount: u64,
) -> Result<()> {
    token::transfer_checked(
        CpiContext::new_with_signer(
            token_program.key(),
            TransferChecked {
                from: from.to_account_info(),
                mint: mint.to_account_info(),
                to: to.to_account_info(),
                authority: pool.to_account_info(),
            },
            signer,
        ),
        amount,
        mint.decimals,
    )
}

#[derive(Accounts)]
pub struct InitializeInKindPool<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    pub asset_a_mint: Box<Account<'info, Mint>>,
    #[account(constraint = asset_b_mint.decimals == asset_a_mint.decimals)]
    pub asset_b_mint: Box<Account<'info, Mint>>,
    #[account(
        init,
        payer = authority,
        space = 8 + InKindPool::INIT_SPACE,
        seeds = [
            IN_KIND_POOL_SEED,
            authority.key().as_ref(),
            asset_a_mint.key().as_ref(),
            asset_b_mint.key().as_ref()
        ],
        bump
    )]
    pub pool: Box<Account<'info, InKindPool>>,
    #[account(
        init,
        payer = authority,
        mint::decimals = asset_a_mint.decimals,
        mint::authority = pool,
        seeds = [SHARE_MINT_SEED, pool.key().as_ref()],
        bump
    )]
    pub share_mint: Box<Account<'info, Mint>>,
    #[account(
        init,
        payer = authority,
        token::mint = asset_a_mint,
        token::authority = pool,
        seeds = [ASSET_A_VAULT_SEED, pool.key().as_ref()],
        bump
    )]
    pub asset_a_vault: Box<Account<'info, TokenAccount>>,
    #[account(
        init,
        payer = authority,
        token::mint = asset_b_mint,
        token::authority = pool,
        seeds = [ASSET_B_VAULT_SEED, pool.key().as_ref()],
        bump
    )]
    pub asset_b_vault: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DepositInKind<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        seeds = [
            IN_KIND_POOL_SEED,
            pool.authority.as_ref(),
            pool.asset_a_mint.as_ref(),
            pool.asset_b_mint.as_ref()
        ],
        bump = pool.bump,
        has_one = asset_a_mint,
        has_one = asset_b_mint,
        has_one = share_mint
    )]
    pub pool: Box<Account<'info, InKindPool>>,
    pub asset_a_mint: Box<Account<'info, Mint>>,
    pub asset_b_mint: Box<Account<'info, Mint>>,
    #[account(mut, mint::authority = pool)]
    pub share_mint: Box<Account<'info, Mint>>,
    #[account(
        mut,
        token::mint = asset_a_mint,
        token::authority = pool,
        seeds = [ASSET_A_VAULT_SEED, pool.key().as_ref()],
        bump
    )]
    pub asset_a_vault: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        token::mint = asset_b_mint,
        token::authority = pool,
        seeds = [ASSET_B_VAULT_SEED, pool.key().as_ref()],
        bump
    )]
    pub asset_b_vault: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::mint = asset_a_mint, token::authority = user)]
    pub user_asset_a: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::mint = asset_b_mint, token::authority = user)]
    pub user_asset_b: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::mint = share_mint, token::authority = user)]
    pub user_shares: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(shares: u64)]
pub struct RedeemInKind<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        seeds = [
            IN_KIND_POOL_SEED,
            pool.authority.as_ref(),
            pool.asset_a_mint.as_ref(),
            pool.asset_b_mint.as_ref()
        ],
        bump = pool.bump,
        has_one = asset_a_mint,
        has_one = asset_b_mint,
        has_one = share_mint
    )]
    pub pool: Box<Account<'info, InKindPool>>,
    pub asset_a_mint: Box<Account<'info, Mint>>,
    pub asset_b_mint: Box<Account<'info, Mint>>,
    #[account(mut, mint::authority = pool)]
    pub share_mint: Box<Account<'info, Mint>>,
    #[account(
        mut,
        token::mint = asset_a_mint,
        token::authority = pool,
        seeds = [ASSET_A_VAULT_SEED, pool.key().as_ref()],
        bump
    )]
    pub asset_a_vault: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        token::mint = asset_b_mint,
        token::authority = pool,
        seeds = [ASSET_B_VAULT_SEED, pool.key().as_ref()],
        bump
    )]
    pub asset_b_vault: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::mint = asset_a_mint, token::authority = user)]
    pub user_asset_a: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::mint = asset_b_mint, token::authority = user)]
    pub user_asset_b: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        token::mint = share_mint,
        token::authority = user,
        constraint = user_shares.amount >= shares @ VaultEscrowError::InsufficientTokens
    )]
    pub user_shares: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}
