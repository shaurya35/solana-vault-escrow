mod common;

use anchor_lang::{
    prelude::Pubkey, solana_program::instruction::Instruction, InstructionData, ToAccountMetas,
};
use common::*;
use solana_keypair::Keypair;
use solana_signer::Signer;
use solana_vault_escrow as program;

struct PoolAddresses {
    pool: Pubkey,
    share_mint: Pubkey,
    asset_a_vault: Pubkey,
    asset_b_vault: Pubkey,
}

fn pool_addresses(authority: &Pubkey, asset_a: &Pubkey, asset_b: &Pubkey) -> PoolAddresses {
    let (pool, _) = Pubkey::find_program_address(
        &[
            program::IN_KIND_POOL_SEED,
            authority.as_ref(),
            asset_a.as_ref(),
            asset_b.as_ref(),
        ],
        &program::id(),
    );
    let (share_mint, _) =
        Pubkey::find_program_address(&[program::SHARE_MINT_SEED, pool.as_ref()], &program::id());
    let (asset_a_vault, _) = Pubkey::find_program_address(
        &[program::ASSET_A_VAULT_SEED, pool.as_ref()],
        &program::id(),
    );
    let (asset_b_vault, _) = Pubkey::find_program_address(
        &[program::ASSET_B_VAULT_SEED, pool.as_ref()],
        &program::id(),
    );
    PoolAddresses {
        pool,
        share_mint,
        asset_a_vault,
        asset_b_vault,
    }
}

#[test]
fn shares_redeem_pro_rata_for_underlying_assets_at_any_time() {
    let mut svm = new_svm();
    let authority = Keypair::new();
    let user = Keypair::new();
    let attacker = Keypair::new();
    fund(&mut svm, &authority);
    fund(&mut svm, &user);
    fund(&mut svm, &attacker);

    let asset_a = create_mint(&mut svm, &authority, &authority);
    let asset_b = create_mint(&mut svm, &authority, &authority);
    let user_a = create_token_account(&mut svm, &user, &asset_a, &user);
    let user_b = create_token_account(&mut svm, &user, &asset_b, &user);
    mint_to(&mut svm, &authority, &asset_a, &user_a, &authority, 1_000);
    mint_to(&mut svm, &authority, &asset_b, &user_b, &authority, 1_000);

    let addresses = pool_addresses(&authority.pubkey(), &asset_a.pubkey(), &asset_b.pubkey());
    let initialize = Instruction::new_with_bytes(
        program::id(),
        &program::instruction::InitializeInKindPool {}.data(),
        program::accounts::InitializeInKindPool {
            authority: authority.pubkey(),
            asset_a_mint: asset_a.pubkey(),
            asset_b_mint: asset_b.pubkey(),
            pool: addresses.pool,
            share_mint: addresses.share_mint,
            asset_a_vault: addresses.asset_a_vault,
            asset_b_vault: addresses.asset_b_vault,
            token_program: spl_token_interface::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &authority, &[&authority], &[initialize]).unwrap();

    let user_shares = create_token_account_for_mint(&mut svm, &user, &addresses.share_mint, &user);
    let deposit = |amount_a, amount_b| {
        Instruction::new_with_bytes(
            program::id(),
            &program::instruction::DepositInKind { amount_a, amount_b }.data(),
            program::accounts::DepositInKind {
                user: user.pubkey(),
                pool: addresses.pool,
                asset_a_mint: asset_a.pubkey(),
                asset_b_mint: asset_b.pubkey(),
                share_mint: addresses.share_mint,
                asset_a_vault: addresses.asset_a_vault,
                asset_b_vault: addresses.asset_b_vault,
                user_asset_a: user_a.pubkey(),
                user_asset_b: user_b.pubkey(),
                user_shares: user_shares.pubkey(),
                token_program: spl_token_interface::ID,
            }
            .to_account_metas(None),
        )
    };
    send(&mut svm, &user, &[&user], &[deposit(400, 400)]).unwrap();

    assert_eq!(token_balance(&svm, &user_a), 600);
    assert_eq!(token_balance(&svm, &user_b), 600);
    assert_eq!(token_balance_at(&svm, &addresses.asset_a_vault), 400);
    assert_eq!(token_balance_at(&svm, &addresses.asset_b_vault), 400);
    assert_eq!(token_balance(&svm, &user_shares), 400);
    assert_eq!(mint_supply(&svm, &addresses.share_mint), 400);

    assert!(send(&mut svm, &user, &[&user], &[deposit(100, 200)]).is_err());
    assert_eq!(token_balance(&svm, &user_a), 600);
    assert_eq!(token_balance(&svm, &user_b), 600);

    let redeem = |user_key: Pubkey, user_a_key: Pubkey, user_b_key: Pubkey, shares| {
        Instruction::new_with_bytes(
            program::id(),
            &program::instruction::RedeemInKind { shares }.data(),
            program::accounts::RedeemInKind {
                user: user_key,
                pool: addresses.pool,
                asset_a_mint: asset_a.pubkey(),
                asset_b_mint: asset_b.pubkey(),
                share_mint: addresses.share_mint,
                asset_a_vault: addresses.asset_a_vault,
                asset_b_vault: addresses.asset_b_vault,
                user_asset_a: user_a_key,
                user_asset_b: user_b_key,
                user_shares: user_shares.pubkey(),
                token_program: spl_token_interface::ID,
            }
            .to_account_metas(None),
        )
    };

    let attacker_a = create_token_account(&mut svm, &attacker, &asset_a, &attacker);
    let attacker_b = create_token_account(&mut svm, &attacker, &asset_b, &attacker);
    assert!(send(
        &mut svm,
        &attacker,
        &[&attacker],
        &[redeem(
            attacker.pubkey(),
            attacker_a.pubkey(),
            attacker_b.pubkey(),
            1,
        )],
    )
    .is_err());

    send(
        &mut svm,
        &user,
        &[&user],
        &[redeem(user.pubkey(), user_a.pubkey(), user_b.pubkey(), 100)],
    )
    .unwrap();
    assert_eq!(token_balance(&svm, &user_a), 700);
    assert_eq!(token_balance(&svm, &user_b), 700);
    assert_eq!(token_balance(&svm, &user_shares), 300);
    assert_eq!(token_balance_at(&svm, &addresses.asset_a_vault), 300);
    assert_eq!(token_balance_at(&svm, &addresses.asset_b_vault), 300);

    send(
        &mut svm,
        &user,
        &[&user],
        &[redeem(user.pubkey(), user_a.pubkey(), user_b.pubkey(), 300)],
    )
    .unwrap();
    assert_eq!(token_balance(&svm, &user_a), 1_000);
    assert_eq!(token_balance(&svm, &user_b), 1_000);
    assert_eq!(token_balance(&svm, &user_shares), 0);
    assert_eq!(mint_supply(&svm, &addresses.share_mint), 0);
    assert_eq!(token_balance_at(&svm, &addresses.asset_a_vault), 0);
    assert_eq!(token_balance_at(&svm, &addresses.asset_b_vault), 0);
}
