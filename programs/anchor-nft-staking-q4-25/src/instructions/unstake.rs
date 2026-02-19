use anchor_lang::prelude::*;
use mpl_core::{
    instructions::RemovePluginV1CpiBuilder,
    types::{PluginType, FreezeDelegate, Plugin},
    ID as CORE_PROGRAM_ID,
};

use crate::{
    errors::StakeError,
    state::{StakeAccount, StakeConfig, UserAccount, CollectionInfo},
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
        mut,
        constraint = collection.owner == &CORE_PROGRAM_ID @ StakeError::InvalidCollection,
    )]
    /// CHECK: Metaplex Core Collection
    pub collection: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"collection_info", collection.key().as_ref()],
        bump = collection_info.bump,
    )]
    pub collection_info: Account<'info, CollectionInfo>,

    #[account(
        mut,
        close = user,
        seeds = [b"stake", config.key().as_ref(), asset.key().as_ref()],
        bump = stake_account.bump,
        constraint = stake_account.owner == user.key() @ StakeError::NotOwner,
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

        // Fix 2: compare elapsed days, not raw seconds, against freeze_period
        require!(
            time_elapsed / 86400 >= self.config.freeze_period as i64,
            StakeError::FreezePeriodNotPassed
        );

        self.user_account.points += ((time_elapsed as u32) * self.config.points_per_stake as u32) / 86400;

        let signer_seeds: &[&[&[u8]]] = &[&[
            b"stake",
            self.config.to_account_info().key.as_ref(),
            self.asset.to_account_info().key.as_ref(),
            &[self.stake_account.bump],
        ]];

        // Fix 3: unfreeze with stake PDA as authority (it was set as init_authority at stake time)
        mpl_core::instructions::UpdatePluginV1CpiBuilder::new(&self.core_program.to_account_info())
            .asset(&self.asset.to_account_info())
            .collection(Some(&self.collection.to_account_info()))
            .payer(&self.user.to_account_info())
            .authority(Some(&self.stake_account.to_account_info()))
            .system_program(&self.system_program.to_account_info())
            .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: false }))
            .invoke_signed(signer_seeds)?;

        // Remove the FreezeDelegate plugin — RemovePlugin requires the asset owner (user), not
        // the plugin's registered authority. The plugin was already unfrozen above.
        RemovePluginV1CpiBuilder::new(&self.core_program.to_account_info())
            .asset(&self.asset.to_account_info())
            .collection(Some(&self.collection.to_account_info()))
            .payer(&self.user.to_account_info())
            .authority(Some(&self.user.to_account_info()))
            .system_program(&self.system_program.to_account_info())
            .plugin_type(PluginType::FreezeDelegate)
            .invoke()?;

        self.user_account.amount_staked -= 1;

        Ok(())
    }
}
