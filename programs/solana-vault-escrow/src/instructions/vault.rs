use anchor_lang::{
    prelude::*,
    system_program::{transfer, Transfer},
};

use crate::{VaultError, VaultState, STATE_SEED, VAULT_SEED};

pub(crate) fn initialize(ctx: Context<Initialize>) -> Result<()> {
    let state = &mut ctx.accounts.state;

    state.owner = ctx.accounts.owner.key();
    state.state_bump = ctx.bumps.state;
    state.vault_bump = ctx.bumps.vault;

    Ok(())
}

pub(crate) fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
    require!(amount > 0, VaultError::ZeroAmount);

    let cpi_accounts = Transfer {
        from: ctx.accounts.owner.to_account_info(),
        to: ctx.accounts.vault.to_account_info(),
    };

    transfer(
        CpiContext::new(ctx.accounts.system_program.key(), cpi_accounts),
        amount,
    )
}

pub(crate) fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
    require!(amount > 0, VaultError::ZeroAmount);
    require!(
        amount <= ctx.accounts.vault.to_account_info().lamports(),
        VaultError::InsufficientFunds
    );

    transfer_from_vault(
        &ctx.accounts.state,
        &ctx.accounts.vault,
        &ctx.accounts.owner.to_account_info(),
        &ctx.accounts.system_program,
        amount,
    )
}

pub(crate) fn close(ctx: Context<CloseVault>) -> Result<()> {
    let amount = ctx.accounts.vault.to_account_info().lamports();

    if amount > 0 {
        transfer_from_vault(
            &ctx.accounts.state,
            &ctx.accounts.vault,
            &ctx.accounts.owner.to_account_info(),
            &ctx.accounts.system_program,
            amount,
        )?;
    }

    Ok(())
}

fn transfer_from_vault<'info>(
    state: &Account<'info, VaultState>,
    vault: &SystemAccount<'info>,
    owner: &AccountInfo<'info>,
    system_program: &Program<'info, System>,
    amount: u64,
) -> Result<()> {
    let state_key = state.key();
    let signer_seeds: &[&[&[u8]]] = &[&[VAULT_SEED, state_key.as_ref(), &[state.vault_bump]]];

    let cpi_accounts = Transfer {
        from: vault.to_account_info(),
        to: owner.clone(),
    };

    transfer(
        CpiContext::new_with_signer(system_program.key(), cpi_accounts, signer_seeds),
        amount,
    )
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        init,
        payer = owner,
        space = 8 + VaultState::INIT_SPACE,
        seeds = [STATE_SEED, owner.key().as_ref()],
        bump,
    )]
    pub state: Account<'info, VaultState>,
    #[account(seeds = [VAULT_SEED, state.key().as_ref()], bump)]
    pub vault: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        seeds = [STATE_SEED, owner.key().as_ref()],
        bump = state.state_bump,
        has_one = owner @ VaultError::Unauthorized
    )]
    pub state: Account<'info, VaultState>,
    #[account(
        mut,
        seeds = [VAULT_SEED, state.key().as_ref()],
        bump = state.vault_bump
    )]
    pub vault: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        seeds = [STATE_SEED, owner.key().as_ref()],
        bump = state.state_bump,
        has_one = owner @ VaultError::Unauthorized
    )]
    pub state: Account<'info, VaultState>,
    #[account(
        mut,
        seeds = [VAULT_SEED, state.key().as_ref()],
        bump = state.vault_bump
    )]
    pub vault: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CloseVault<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        mut,
        close = owner,
        seeds = [STATE_SEED, owner.key().as_ref()],
        bump = state.state_bump,
        has_one = owner @ VaultError::Unauthorized
    )]
    pub state: Account<'info, VaultState>,
    #[account(
        mut,
        seeds = [VAULT_SEED, state.key().as_ref()],
        bump = state.vault_bump
    )]
    pub vault: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}
