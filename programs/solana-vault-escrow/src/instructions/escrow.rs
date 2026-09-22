use anchor_lang::prelude::*;
use anchor_spl::token::{self, CloseAccount, Mint, Token, TokenAccount, TransferChecked};

use crate::{EscrowError, Offer, OFFER_SEED, VAULT_SEED};

pub(crate) fn make(
    ctx: Context<Make>,
    id: u64,
    amount_a: u64,
    amount_b: u64,
    taker: Pubkey,
    expires_at: i64,
) -> Result<()> {
    require!(amount_a > 0 && amount_b > 0, EscrowError::ZeroAmount);
    validate_new_expiry(expires_at)?;

    let offer = &mut ctx.accounts.offer;
    offer.maker = ctx.accounts.maker.key();
    offer.taker = taker;
    offer.mint_a = ctx.accounts.mint_a.key();
    offer.mint_b = ctx.accounts.mint_b.key();
    offer.id = id;
    offer.amount_a = amount_a;
    offer.amount_b = amount_b;
    offer.expires_at = expires_at;
    offer.offer_bump = ctx.bumps.offer;
    offer.vault_bump = ctx.bumps.vault;

    let accounts = TransferChecked {
        from: ctx.accounts.maker_a.to_account_info(),
        mint: ctx.accounts.mint_a.to_account_info(),
        to: ctx.accounts.vault.to_account_info(),
        authority: ctx.accounts.maker.to_account_info(),
    };

    token::transfer_checked(
        CpiContext::new(ctx.accounts.token_program.key(), accounts),
        amount_a,
        ctx.accounts.mint_a.decimals,
    )
}

pub(crate) fn update(
    ctx: Context<UpdateOffer>,
    amount_b: u64,
    taker: Pubkey,
    expires_at: i64,
) -> Result<()> {
    require!(amount_b > 0, EscrowError::ZeroAmount);

    let now = Clock::get()?.unix_timestamp;
    let old_expiry = ctx.accounts.offer.expires_at;

    require!(old_expiry == 0 || now <= old_expiry, EscrowError::Expired);
    validate_new_expiry(expires_at)?;

    let offer = &mut ctx.accounts.offer;
    offer.amount_b = amount_b;
    offer.taker = taker;
    offer.expires_at = expires_at;

    Ok(())
}

pub(crate) fn take(ctx: Context<Take>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let offer = &ctx.accounts.offer;

    require!(
        offer.expires_at == 0 || now <= offer.expires_at,
        EscrowError::Expired
    );
    require!(
        offer.taker == Pubkey::default() || offer.taker == ctx.accounts.taker.key(),
        EscrowError::WrongTaker
    );

    let pay_maker = TransferChecked {
        from: ctx.accounts.taker_b.to_account_info(),
        mint: ctx.accounts.mint_b.to_account_info(),
        to: ctx.accounts.maker_b.to_account_info(),
        authority: ctx.accounts.taker.to_account_info(),
    };

    token::transfer_checked(
        CpiContext::new(ctx.accounts.token_program.key(), pay_maker),
        offer.amount_b,
        ctx.accounts.mint_b.decimals,
    )?;

    transfer_a_and_close_vault(
        offer,
        &ctx.accounts.vault,
        &ctx.accounts.mint_a,
        &ctx.accounts.taker_a,
        &ctx.accounts.maker.to_account_info(),
        &ctx.accounts.token_program,
    )
}

pub(crate) fn refund(ctx: Context<Refund>) -> Result<()> {
    let offer = &ctx.accounts.offer;
    let now = Clock::get()?.unix_timestamp;

    require!(
        offer.expires_at == 0 || now > offer.expires_at,
        EscrowError::NotExpired
    );

    transfer_a_and_close_vault(
        offer,
        &ctx.accounts.vault,
        &ctx.accounts.mint_a,
        &ctx.accounts.maker_a,
        &ctx.accounts.maker.to_account_info(),
        &ctx.accounts.token_program,
    )
}

fn validate_new_expiry(expires_at: i64) -> Result<()> {
    if expires_at != 0 {
        require!(
            expires_at > Clock::get()?.unix_timestamp,
            EscrowError::BadExpiry
        );
    }

    Ok(())
}

fn transfer_a_and_close_vault<'info>(
    offer: &Account<'info, Offer>,
    vault: &Account<'info, TokenAccount>,
    mint_a: &Account<'info, Mint>,
    destination: &Account<'info, TokenAccount>,
    rent_recipient: &AccountInfo<'info>,
    token_program: &Program<'info, Token>,
) -> Result<()> {
    let offer_key = offer.key();
    let signer_seeds: &[&[&[u8]]] = &[&[VAULT_SEED, offer_key.as_ref(), &[offer.vault_bump]]];

    let transfer_accounts = TransferChecked {
        from: vault.to_account_info(),
        mint: mint_a.to_account_info(),
        to: destination.to_account_info(),
        authority: vault.to_account_info(),
    };

    token::transfer_checked(
        CpiContext::new_with_signer(token_program.key(), transfer_accounts, signer_seeds),
        offer.amount_a,
        mint_a.decimals,
    )?;

    let close_accounts = CloseAccount {
        account: vault.to_account_info(),
        destination: rent_recipient.clone(),
        authority: vault.to_account_info(),
    };

    token::close_account(CpiContext::new_with_signer(
        token_program.key(),
        close_accounts,
        signer_seeds,
    ))
}

#[derive(Accounts)]
#[instruction(id: u64, amount_a: u64)]
pub struct Make<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,
    #[account(
        init,
        payer = maker,
        space = 8 + Offer::INIT_SPACE,
        seeds = [OFFER_SEED, maker.key().as_ref(), &id.to_le_bytes()],
        bump
    )]
    pub offer: Box<Account<'info, Offer>>,
    pub mint_a: Box<Account<'info, Mint>>,
    pub mint_b: Box<Account<'info, Mint>>,
    #[account(
        mut,
        token::mint = mint_a,
        token::authority = maker,
        constraint = maker_a.amount >= amount_a @ EscrowError::InsufficientTokens
    )]
    pub maker_a: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = maker,
        token::mint = mint_a,
        token::authority = vault,
        seeds = [VAULT_SEED, offer.key().as_ref()],
        bump
    )]
    pub vault: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateOffer<'info> {
    pub maker: Signer<'info>,
    #[account(
        mut,
        has_one = maker @ EscrowError::Unauthorized,
        seeds = [OFFER_SEED, maker.key().as_ref(), &offer.id.to_le_bytes()],
        bump = offer.offer_bump
    )]
    pub offer: Account<'info, Offer>,
}

#[derive(Accounts)]
pub struct Take<'info> {
    #[account(mut)]
    pub taker: Signer<'info>,
    /// CHECK: The address constraint requires this account to equal the maker stored in the offer.
    #[account(mut, address = offer.maker)]
    pub maker: UncheckedAccount<'info>,
    #[account(
        mut,
        close = maker,
        has_one = mint_a,
        has_one = mint_b,
        seeds = [OFFER_SEED, offer.maker.as_ref(), &offer.id.to_le_bytes()],
        bump = offer.offer_bump
    )]
    pub offer: Account<'info, Offer>,
    pub mint_a: Account<'info, Mint>,
    pub mint_b: Account<'info, Mint>,
    #[account(
        mut,
        token::mint = mint_a,
        token::authority = vault,
        seeds = [VAULT_SEED, offer.key().as_ref()],
        bump = offer.vault_bump
    )]
    pub vault: Account<'info, TokenAccount>,
    #[account(mut, token::mint = mint_a, token::authority = taker)]
    pub taker_a: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        token::mint = mint_b,
        token::authority = taker,
        constraint = taker_b.amount >= offer.amount_b @ EscrowError::InsufficientTokens
    )]
    pub taker_b: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::mint = mint_b, token::authority = maker)]
    pub maker_b: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Refund<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,
    #[account(
        mut,
        close = maker,
        has_one = maker @ EscrowError::Unauthorized,
        has_one = mint_a,
        seeds = [OFFER_SEED, maker.key().as_ref(), &offer.id.to_le_bytes()],
        bump = offer.offer_bump
    )]
    pub offer: Account<'info, Offer>,
    pub mint_a: Account<'info, Mint>,
    #[account(
        mut,
        token::mint = mint_a,
        token::authority = vault,
        seeds = [VAULT_SEED, offer.key().as_ref()],
        bump = offer.vault_bump
    )]
    pub vault: Account<'info, TokenAccount>,
    #[account(mut, token::mint = mint_a, token::authority = maker)]
    pub maker_a: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}
