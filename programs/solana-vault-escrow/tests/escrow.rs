mod common;

use anchor_lang::{
    prelude::Pubkey, solana_program::instruction::Instruction, InstructionData, ToAccountMetas,
};
use common::*;
use solana_clock::Clock;
use solana_keypair::Keypair;
use solana_signer::Signer;
use solana_vault_escrow as program;

fn offer_addresses(maker: &Pubkey, id: u64) -> (Pubkey, Pubkey) {
    let (offer, _) = Pubkey::find_program_address(
        &[program::OFFER_SEED, maker.as_ref(), &id.to_le_bytes()],
        &program::id(),
    );
    let (vault, _) =
        Pubkey::find_program_address(&[program::VAULT_SEED, offer.as_ref()], &program::id());
    (offer, vault)
}

#[test]
fn make_update_take_enforces_designated_taker_and_settles_atomically() {
    let mut svm = new_svm();
    let maker = Keypair::new();
    let taker = Keypair::new();
    let attacker = Keypair::new();
    fund(&mut svm, &maker);
    fund(&mut svm, &taker);
    fund(&mut svm, &attacker);

    let mint_a = create_mint(&mut svm, &maker, &maker);
    let mint_b = create_mint(&mut svm, &maker, &maker);
    let maker_a = create_token_account(&mut svm, &maker, &mint_a, &maker);
    let maker_b = create_token_account(&mut svm, &maker, &mint_b, &maker);
    let taker_a = create_token_account(&mut svm, &taker, &mint_a, &taker);
    let taker_b = create_token_account(&mut svm, &taker, &mint_b, &taker);
    let attacker_a = create_token_account(&mut svm, &attacker, &mint_a, &attacker);
    let attacker_b = create_token_account(&mut svm, &attacker, &mint_b, &attacker);
    mint_to(&mut svm, &maker, &mint_a, &maker_a, &maker, 1_000);
    mint_to(&mut svm, &maker, &mint_b, &taker_b, &maker, 2_000);
    mint_to(&mut svm, &maker, &mint_b, &attacker_b, &maker, 2_000);

    let clock: Clock = svm.get_sysvar();
    let id = 7;
    let (offer, vault) = offer_addresses(&maker.pubkey(), id);
    let make = Instruction::new_with_bytes(
        program::id(),
        &program::instruction::Make {
            id,
            amount_a: 400,
            amount_b: 250,
            taker: taker.pubkey(),
            expires_at: clock.unix_timestamp + 100,
        }
        .data(),
        program::accounts::Make {
            maker: maker.pubkey(),
            offer,
            mint_a: mint_a.pubkey(),
            mint_b: mint_b.pubkey(),
            maker_a: maker_a.pubkey(),
            vault,
            token_program: spl_token_interface::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &maker, &[&maker], &[make]).unwrap();
    assert_eq!(token_balance(&svm, &maker_a), 600);
    assert_eq!(token_balance_at(&svm, &vault), 400);

    let unauthorized_update = Instruction::new_with_bytes(
        program::id(),
        &program::instruction::Update {
            amount_b: 1,
            taker: attacker.pubkey(),
            expires_at: clock.unix_timestamp + 200,
        }
        .data(),
        program::accounts::UpdateOffer {
            maker: attacker.pubkey(),
            offer,
        }
        .to_account_metas(None),
    );
    assert!(send(&mut svm, &attacker, &[&attacker], &[unauthorized_update]).is_err());

    let update = Instruction::new_with_bytes(
        program::id(),
        &program::instruction::Update {
            amount_b: 300,
            taker: taker.pubkey(),
            expires_at: clock.unix_timestamp + 200,
        }
        .data(),
        program::accounts::UpdateOffer {
            maker: maker.pubkey(),
            offer,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &maker, &[&maker], &[update]).unwrap();

    let attacker_take = Instruction::new_with_bytes(
        program::id(),
        &program::instruction::Take {}.data(),
        program::accounts::Take {
            taker: attacker.pubkey(),
            maker: maker.pubkey(),
            offer,
            mint_a: mint_a.pubkey(),
            mint_b: mint_b.pubkey(),
            vault,
            taker_a: attacker_a.pubkey(),
            taker_b: attacker_b.pubkey(),
            maker_b: maker_b.pubkey(),
            token_program: spl_token_interface::ID,
        }
        .to_account_metas(None),
    );
    assert!(send(&mut svm, &attacker, &[&attacker], &[attacker_take]).is_err());
    assert_eq!(token_balance_at(&svm, &vault), 400);
    assert_eq!(token_balance(&svm, &maker_b), 0);

    let take = Instruction::new_with_bytes(
        program::id(),
        &program::instruction::Take {}.data(),
        program::accounts::Take {
            taker: taker.pubkey(),
            maker: maker.pubkey(),
            offer,
            mint_a: mint_a.pubkey(),
            mint_b: mint_b.pubkey(),
            vault,
            taker_a: taker_a.pubkey(),
            taker_b: taker_b.pubkey(),
            maker_b: maker_b.pubkey(),
            token_program: spl_token_interface::ID,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &taker, &[&taker], &[take]).unwrap();

    assert_eq!(token_balance(&svm, &taker_a), 400);
    assert_eq!(token_balance(&svm, &taker_b), 1_700);
    assert_eq!(token_balance(&svm, &maker_b), 300);
    assert!(svm.get_account(&offer).is_none());
    assert!(svm.get_account(&vault).is_none());
}

#[test]
fn timed_offer_blocks_early_refund_and_take_after_expiry_then_refunds() {
    let mut svm = new_svm();
    let maker = Keypair::new();
    let taker = Keypair::new();
    fund(&mut svm, &maker);
    fund(&mut svm, &taker);

    let mint_a = create_mint(&mut svm, &maker, &maker);
    let mint_b = create_mint(&mut svm, &maker, &maker);
    let maker_a = create_token_account(&mut svm, &maker, &mint_a, &maker);
    let maker_b = create_token_account(&mut svm, &maker, &mint_b, &maker);
    let taker_a = create_token_account(&mut svm, &taker, &mint_a, &taker);
    let taker_b = create_token_account(&mut svm, &taker, &mint_b, &taker);
    mint_to(&mut svm, &maker, &mint_a, &maker_a, &maker, 500);
    mint_to(&mut svm, &maker, &mint_b, &taker_b, &maker, 500);

    let mut clock: Clock = svm.get_sysvar();
    let expires_at = clock.unix_timestamp + 10;
    let id = 9;
    let (offer, vault) = offer_addresses(&maker.pubkey(), id);
    let make = Instruction::new_with_bytes(
        program::id(),
        &program::instruction::Make {
            id,
            amount_a: 200,
            amount_b: 100,
            taker: taker.pubkey(),
            expires_at,
        }
        .data(),
        program::accounts::Make {
            maker: maker.pubkey(),
            offer,
            mint_a: mint_a.pubkey(),
            mint_b: mint_b.pubkey(),
            maker_a: maker_a.pubkey(),
            vault,
            token_program: spl_token_interface::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &maker, &[&maker], &[make]).unwrap();

    let refund = || {
        Instruction::new_with_bytes(
            program::id(),
            &program::instruction::Refund {}.data(),
            program::accounts::Refund {
                maker: maker.pubkey(),
                offer,
                mint_a: mint_a.pubkey(),
                vault,
                maker_a: maker_a.pubkey(),
                token_program: spl_token_interface::ID,
            }
            .to_account_metas(None),
        )
    };
    assert!(send(&mut svm, &maker, &[&maker], &[refund()]).is_err());
    assert_eq!(token_balance_at(&svm, &vault), 200);

    clock.unix_timestamp = expires_at + 1;
    svm.set_sysvar(&clock);

    let take = Instruction::new_with_bytes(
        program::id(),
        &program::instruction::Take {}.data(),
        program::accounts::Take {
            taker: taker.pubkey(),
            maker: maker.pubkey(),
            offer,
            mint_a: mint_a.pubkey(),
            mint_b: mint_b.pubkey(),
            vault,
            taker_a: taker_a.pubkey(),
            taker_b: taker_b.pubkey(),
            maker_b: maker_b.pubkey(),
            token_program: spl_token_interface::ID,
        }
        .to_account_metas(None),
    );
    assert!(send(&mut svm, &taker, &[&taker], &[take]).is_err());
    send(&mut svm, &maker, &[&maker], &[refund()]).unwrap();

    assert_eq!(token_balance(&svm, &maker_a), 500);
    assert!(svm.get_account(&offer).is_none());
    assert!(svm.get_account(&vault).is_none());
}
