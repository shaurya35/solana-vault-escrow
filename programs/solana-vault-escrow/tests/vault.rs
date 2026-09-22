use solana_vault_escrow as vault;
use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{instruction::Instruction, system_program},
        InstructionData, ToAccountMetas,
    },
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

fn send(
    svm: &mut LiteSVM,
    payer: &Keypair,
    signers: &[&Keypair],
    instruction: Instruction,
) -> Result<(), String> {
    let message = Message::new_with_blockhash(
        &[instruction],
        Some(&payer.pubkey()),
        &svm.latest_blockhash(),
    );

    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(message), signers)
        .map_err(|e| e.to_string())?;

    let result = svm.send_transaction(tx);
    svm.expire_blockhash();

    result.map(|_| ()).map_err(|e| format!("{e:?}"))
}

#[test]
fn initialize_deposit_withdraw_reject_attacker_and_close() {
    let program_id = vault::id();
    let owner = Keypair::new();
    let attacker = Keypair::new();

    let (state, _) =
        Pubkey::find_program_address(&[vault::STATE_SEED, owner.pubkey().as_ref()], &program_id);

    let (vault_pda, _) =
        Pubkey::find_program_address(&[vault::VAULT_SEED, state.as_ref()], &program_id);

    let mut svm = LiteSVM::new();

    svm.add_program_from_file(
        program_id,
        concat!(
            env!("CARGO_TARGET_TMPDIR"),
            "/../deploy/solana_vault_escrow.so"
        ),
    )
    .unwrap();

    svm.airdrop(&owner.pubkey(), 2_000_000_000).unwrap();
    svm.airdrop(&attacker.pubkey(), 2_000_000_000).unwrap();

    let initialize = Instruction::new_with_bytes(
        program_id,
        &vault::instruction::Initialize {}.data(),
        vault::accounts::Initialize {
            owner: owner.pubkey(),
            state,
            vault: vault_pda,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    );

    send(&mut svm, &owner, &[&owner], initialize).unwrap();

    let deposit = Instruction::new_with_bytes(
        program_id,
        &vault::instruction::Deposit {
            amount: 200_000_000,
        }
        .data(),
        vault::accounts::Deposit {
            owner: owner.pubkey(),
            state,
            vault: vault_pda,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    );

    send(&mut svm, &owner, &[&owner], deposit).unwrap();
    assert_eq!(svm.get_account(&vault_pda).unwrap().lamports, 200_000_000);

    let withdraw = Instruction::new_with_bytes(
        program_id,
        &vault::instruction::Withdraw { amount: 80_000_000 }.data(),
        vault::accounts::Withdraw {
            owner: owner.pubkey(),
            state,
            vault: vault_pda,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    );

    send(&mut svm, &owner, &[&owner], withdraw).unwrap();
    assert_eq!(svm.get_account(&vault_pda).unwrap().lamports, 120_000_000);

    let attacker_withdraw = Instruction::new_with_bytes(
        program_id,
        &vault::instruction::Withdraw { amount: 1 }.data(),
        vault::accounts::Withdraw {
            owner: attacker.pubkey(),
            state,
            vault: vault_pda,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    );

    assert!(send(&mut svm, &attacker, &[&attacker], attacker_withdraw).is_err());
    assert_eq!(svm.get_account(&vault_pda).unwrap().lamports, 120_000_000);

    let close = Instruction::new_with_bytes(
        program_id,
        &vault::instruction::Close {}.data(),
        vault::accounts::CloseVault {
            owner: owner.pubkey(),
            state,
            vault: vault_pda,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    );

    send(&mut svm, &owner, &[&owner], close).unwrap();
    assert!(svm.get_account(&state).is_none());
    assert!(svm
        .get_account(&vault_pda)
        .map(|account| account.lamports == 0)
        .unwrap_or(true));
}
