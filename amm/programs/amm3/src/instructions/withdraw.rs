use crate::error::AmmError;
use crate::state::Config;
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{burn, transfer, Burn, Mint, MintTo, Token, TokenAccount, Transfer},
};
use constant_product_curve::ConstantProduct;

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    pub mint_x: Account<'info, Mint>,
    pub mint_y: Account<'info, Mint>,
    #[account(
        has_one = mint_x,
        has_one = mint_y,
        seeds= [b"config", config.seed.to_le_bytes().as_ref()],
        bump=config.config_bump,
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut, 
        seeds= [b"lp", config.key().as_ref()],
        bump= config.lp_bump,
    )]
    pub mint_lp: Account<'info, Mint>,
    #[account(
        mut,
        associated_token::mint= mint_x,
        associated_token::authority=config,
    )]
    pub vault_x: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint= mint_y,
        associated_token::authority=config,
    )]
    pub vault_y: Account<'info,TokenAccount>,
    #[account(
        init_if_needed,
        payer= user,
        associated_token::mint= mint_x,
        associated_token::authority=user,
    )]
    pub user_x: Account<'info, TokenAccount>,
    #[account(
        init_if_needed,
        payer= user,
        associated_token::mint = mint_y,
        associated_token::authority= user,
    )]
    pub user_y: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint= mint_lp,
        associated_token::authority=user,
    )]
    pub user_lp: Account<'info, TokenAccount>,
    pub system_program: Program<'info,System>,
    pub token_program: Program<'info,Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

impl <'info> Withdraw<'info>{
    pub fn withdraw(
        &mut self, 
        amount:u64,  //amount of lp tokens user wants to burn
        min_x: u64, //min amount of x token user wants to receive
        min_y: u64 // min amount of y token user wants to receive
    )->Result<()>{
        require!(self.config.locked == false, AmmError::PoolLocked);
        require!(amount>0, AmmError::InvalidAmount);
        require!(min_x!=0 && min_y!=0, AmmError::InvalidAmount);
        let amounts = ConstantProduct::xy_withdraw_amounts_from_l(
            self.vault_x.amount,
            self.vault_y.amount, 
            self.mint_lp.supply, 
            amount, 
            6,
        ).map_err(AmmError::from)?;

        require!(min_x<=amounts.x && min_y<=amounts.y, AmmError::SlippageExceded);

        self.withdraw_tokens(true, amounts.x)?;
        self.withdraw_tokens(false, amounts.y)?;
        self.burn_lp_tokens(amount)?;
        Ok(())
    }

    pub fn withdraw_tokens(&self, is_x:bool, amount:u64,)->Result<()>{
        let (from, to) = match is_x {
            true=> (self.vault_x.to_account_info(), self.user_x.to_account_info()),
            false=>(self.vault_y.to_account_info(), self.user_y.to_account_info())
        };
        let program = self.token_program.to_account_info();
        let accounts= Transfer{
            from,
            to,
            authority:self.config.to_account_info(),
        };
        let seeds= &[
            &b"config"[..],
            &self.config.seed.to_le_bytes(),
            &[self.config.config_bump],
        ];
        let signer_seeds = &[&seeds[..]];
        let cpi_ctx= CpiContext::new_with_signer(program, accounts, signer_seeds);
        transfer(cpi_ctx, amount)?;
        Ok(())
    }
    pub fn burn_lp_tokens(&self, amount:u64)->Result<()>{
        let program= self.token_program.to_account_info();
        let accounts= Burn{
            mint: self.mint_lp.to_account_info(),
            from:self.user_lp.to_account_info(),
            authority: self.user.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(program, accounts);
        burn(cpi_ctx, amount)?;
        Ok(())
    }
}
