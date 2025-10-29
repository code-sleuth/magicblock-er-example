use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hash;
use ephemeral_vrf_sdk::anchor::vrf;
use ephemeral_vrf_sdk::consts::{DEFAULT_QUEUE, VRF_PROGRAM_IDENTITY};
use ephemeral_vrf_sdk::instructions::{create_request_randomness_ix, RequestRandomnessParams};
use ephemeral_vrf_sdk::types::SerializableAccountMeta;

use crate::state::UserAccount;

// Request randomness instruction - initiates VRF request
#[vrf]
#[derive(Accounts)]
pub struct RequestVrfRandom<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        seeds = [b"user", user.key().as_ref()],
        bump = user_account.bump,
    )]
    pub user_account: Account<'info, UserAccount>,
    /// CHECK: The oracle queue account for VRF requests
    #[account(mut, address = DEFAULT_QUEUE)]
    pub oracle_queue: AccountInfo<'info>,
}

// Callback instruction receives randomness from VRF oracle
#[derive(Accounts)]
pub struct VrfCallback<'info> {
    /// CHECK: VRF program identity - must be signer for security
    #[account(address = VRF_PROGRAM_IDENTITY)]
    pub vrf_program_identity: Signer<'info>,
    #[account(mut)]
    pub user_account: Account<'info, UserAccount>,
}

impl<'info> RequestVrfRandom<'info> {
    pub fn request_vrf_random(&mut self, caller_seed: u8) -> Result<()> {
        msg!("Requesting randomness with seed: {}", caller_seed);

        // Create the VRF request instruction
        let ix = create_request_randomness_ix(RequestRandomnessParams {
            payer: self.user.key(),
            oracle_queue: self.oracle_queue.key(),
            callback_program_id: crate::ID,
            caller_seed: hash(&[caller_seed]).to_bytes(),
            callback_discriminator: crate::instruction::VrfCallback::DISCRIMINATOR.to_vec(),
            accounts_metas: Some(vec![SerializableAccountMeta {
                pubkey: self.user_account.key(),
                is_signer: false,
                is_writable: true, // Must be writable for callback to update
            }]),
            ..Default::default()
        });

        // Invoke the VRF program using the injected helper from #[vrf] macro
        self.invoke_signed_vrf(&self.user.to_account_info(), &ix)?;

        msg!("VRF request submitted successfully");
        Ok(())
    }
}

impl<'info> VrfCallback<'info> {
    pub fn vrf_callback(&mut self, randomness: [u8; 32]) -> Result<()> {
        msg!("VRF callback received");

        // Verify that VRF program identity is the signer (security check)
        require!(self.vrf_program_identity.is_signer, ErrorCode::Unauthorized);

        // Extract u64 from randomness bytes
        let random_value = u64::from_le_bytes(
            randomness[0..8]
                .try_into()
                .map_err(|_| ErrorCode::InvalidRandomness)?,
        );

        // Update user account with the random value
        self.user_account.randomness_result = random_value;

        msg!("Updated user account with randomness: {}", random_value);
        Ok(())
    }
}

#[error_code]
pub enum ErrorCode {
    #[msg("Unauthorized: VRF program identity must be signer")]
    Unauthorized,
    #[msg("Invalid randomness format")]
    InvalidRandomness,
}
