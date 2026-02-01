use anchor_lang::prelude::*;
use mpl_core::{
    instructions::RemovePluginV1CpiBuilder,
    types::PluginType,
    ID as CORE_PROGRAM_ID,
};

use crate::{
    errors::StakeError,
    state::{StakeAccount, StakeConfig, UserAccount},
};

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = asset.owner == &CORE_PROGRAM_ID @ StakeError::InvalidAsset,
    )]
    /// CHECK: Metaplex Core Asset
    pub asset: UncheckedAccount<'info>,

    #[account(
        constraint = collection.owner == &CORE_PROGRAM_ID @ StakeError::InvalidCollection,
    )]
    /// CHECK: Metaplex Core Collection
    pub collection: UncheckedAccount<'info>,

    #[account(
        mut,
        close = user,
        seeds = [b"stake", config.key().as_ref(), asset.key().as_ref()],
        bump = stake_account.bump,
    )]
    pub stake_account: Box<Account<'info, StakeAccount>>,

    #[account(
        seeds = [b"config"],
        bump = config.bump,
    )]
    pub config: Box<Account<'info, StakeConfig>>,

    #[account(
        mut,
        seeds = [b"user", user.key().as_ref()],
        bump = user_account.bump,
    )]
    pub user_account: Box<Account<'info, UserAccount>>,

    #[account(address = CORE_PROGRAM_ID)]
    /// CHECK: Metaplex Core Program
    pub core_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> Unstake<'info> {
    pub fn unstake(&mut self) -> Result<()> {
        let time_elapsed = Clock::get()?.unix_timestamp - self.stake_account.staked_at;

        require!(
            time_elapsed >= self.config.freeze_period as i64,
            StakeError::FreezePeriodNotPassed
        );

        self.user_account.points +=
            ((time_elapsed as u32) / 86400) * self.config.points_per_stake as u32;

        RemovePluginV1CpiBuilder::new(&self.core_program.to_account_info())
            .asset(&self.asset.to_account_info())
            .collection(Some(&self.collection.to_account_info()))
            .system_program(&self.system_program.to_account_info())
            .plugin_type(PluginType::FreezeDelegate)
            .invoke()?;

        self.user_account.amount_staked -= 1;

        Ok(())
    }
}
