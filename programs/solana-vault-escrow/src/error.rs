use anchor_lang::prelude::*;

#[error_code]
pub enum VaultEscrowError {
    #[msg("The signer is not authorized for this action")]
    Unauthorized,
    #[msg("The amount must be greater than zero")]
    ZeroAmount,
    #[msg("The vault does not contain enough lamports")]
    InsufficientFunds,
    #[msg("The token account does not contain enough tokens")]
    InsufficientTokens,
    #[msg("This signer is not the designated taker")]
    WrongTaker,
    #[msg("The offer has expired")]
    Expired,
    #[msg("The offer has not expired yet")]
    NotExpired,
    #[msg("A nonzero expiry must be in the future")]
    BadExpiry,
}

pub type VaultError = VaultEscrowError;
pub type EscrowError = VaultEscrowError;
