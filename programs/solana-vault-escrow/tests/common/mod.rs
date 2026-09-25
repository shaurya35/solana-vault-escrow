#![allow(dead_code)]

use anchor_lang::solana_program::{instruction::Instruction, system_instruction};
use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_program_pack::Pack;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;
use spl_token_interface::{
    instruction as token_instruction,
    state::{Account as TokenAccount, Mint},
};

pub const DECIMALS: u8 = 6;

pub fn program_path() -> &'static str {
    concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/solana_vault_escrow.so"
    )
}

pub fn new_svm() -> LiteSVM {
    let mut svm = LiteSVM::new();
    svm.add_program_from_file(solana_vault_escrow::id(), program_path())
        .unwrap();
    svm
}

pub fn send(
    svm: &mut LiteSVM,
    payer: &Keypair,
    signers: &[&Keypair],
    instructions: &[Instruction],
) -> Result<(), String> {
    let message =
        Message::new_with_blockhash(instructions, Some(&payer.pubkey()), &svm.latest_blockhash());
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(message), signers)
        .map_err(|error| error.to_string())?;
    let result = svm.send_transaction(tx);
    svm.expire_blockhash();
    result.map(|_| ()).map_err(|error| format!("{error:?}"))
}

pub fn fund(svm: &mut LiteSVM, account: &Keypair) {
    svm.airdrop(&account.pubkey(), 20_000_000_000).unwrap();
}

pub fn create_mint(svm: &mut LiteSVM, payer: &Keypair, authority: &Keypair) -> Keypair {
    let mint = Keypair::new();
    let create = system_instruction::create_account(
        &payer.pubkey(),
        &mint.pubkey(),
        svm.minimum_balance_for_rent_exemption(Mint::LEN),
        Mint::LEN as u64,
        &spl_token_interface::ID,
    );
    let initialize = token_instruction::initialize_mint2(
        &spl_token_interface::ID,
        &mint.pubkey(),
        &authority.pubkey(),
        None,
        DECIMALS,
    )
    .unwrap();
    send(svm, payer, &[payer, &mint], &[create, initialize]).unwrap();
    mint
}

pub fn create_token_account(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    owner: &Keypair,
) -> Keypair {
    create_token_account_for_mint(svm, payer, &mint.pubkey(), owner)
}

pub fn create_token_account_for_mint(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &anchor_lang::prelude::Pubkey,
    owner: &Keypair,
) -> Keypair {
    let account = Keypair::new();
    let create = system_instruction::create_account(
        &payer.pubkey(),
        &account.pubkey(),
        svm.minimum_balance_for_rent_exemption(TokenAccount::LEN),
        TokenAccount::LEN as u64,
        &spl_token_interface::ID,
    );
    let initialize = token_instruction::initialize_account3(
        &spl_token_interface::ID,
        &account.pubkey(),
        mint,
        &owner.pubkey(),
    )
    .unwrap();
    send(svm, payer, &[payer, &account], &[create, initialize]).unwrap();
    account
}

pub fn mint_to(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    destination: &Keypair,
    authority: &Keypair,
    amount: u64,
) {
    let instruction = token_instruction::mint_to(
        &spl_token_interface::ID,
        &mint.pubkey(),
        &destination.pubkey(),
        &authority.pubkey(),
        &[],
        amount,
    )
    .unwrap();
    let signers = if payer.pubkey() == authority.pubkey() {
        vec![payer]
    } else {
        vec![payer, authority]
    };
    send(svm, payer, &signers, &[instruction]).unwrap();
}

pub fn token_balance(svm: &LiteSVM, account: &Keypair) -> u64 {
    let account = svm.get_account(&account.pubkey()).unwrap();
    TokenAccount::unpack(&account.data).unwrap().amount
}

pub fn token_balance_at(svm: &LiteSVM, address: &anchor_lang::prelude::Pubkey) -> u64 {
    let account = svm.get_account(address).unwrap();
    TokenAccount::unpack(&account.data).unwrap().amount
}

pub fn mint_supply(svm: &LiteSVM, address: &anchor_lang::prelude::Pubkey) -> u64 {
    let account = svm.get_account(address).unwrap();
    Mint::unpack(&account.data).unwrap().supply
}
