
use anchor_lang::prelude::*;
use orao_solana_vrf::cpi::accounts::RequestV2;
use orao_solana_vrf::program::OraoVrf;



declare_id!("GZXNehjgHjrVds4vXD16pRHWrNVbD9hyAmUshQYukvbn");

#[program]
pub mod lottery_game {
    use super::*;
    const PARTICIPATION_AMOUNT: u64 = 100_000_000; // 0.1 SOL

    pub fn start_lottery(ctx: Context<StartLottery>, prize_amount: u64) -> Result<()> {
        let lottery = &mut ctx.accounts.lottery;
        lottery.prize_amount = prize_amount;
        lottery.is_active = true;
        Ok(())
    }

    pub fn participate_in_lottery(ctx: Context<ParticipateInLottery>) -> Result<()> {
        let lottery = &mut ctx.accounts.lottery;
        let player = &mut ctx.accounts.player;
    
        if !lottery.is_active {
            return Err(LotteryError::LotteryNotActive.into());
        }
        
        // Send SOL from player to lottery account using fixed amount
        let tr_inst = anchor_lang::solana_program::system_instruction::transfer(
            &player.key(),
            &lottery.key(),
            PARTICIPATION_AMOUNT,
        );
        anchor_lang::solana_program::program::invoke(
            &tr_inst,
            &[
                player.to_account_info(),
                ctx.accounts.lottery_account.to_account_info(),
                ctx.accounts.system_program.to_account_info(),    
            ],
        )?;
        
        lottery.players.push(player.key());
        lottery.prize_amount += PARTICIPATION_AMOUNT;
    
        Ok(())
    }
    
    // choosing the winner
    pub fn draw_winner(ctx: Context<DrawWinner>) -> Result<()> {
        let lottery = &mut ctx.accounts.lottery;
    
        if !lottery.is_active {
            return Err(LotteryError::LotteryNotActive.into());
        }
            
        // Use clock for pseudo-randomness
        let clock = Clock::get()?;
        let random_value = clock.unix_timestamp as u64;
        
        // Use randomness to choose a winner
        let winner_index = random_value % lottery.players.len() as u64;
        let winner = lottery.players[winner_index as usize];
    
        let winner_account = &mut ctx.accounts.winner;
        **winner_account.to_account_info().lamports.borrow_mut() += lottery.prize_amount;
    
        lottery.is_active = false;
    
        Ok(())
    }
    
    pub fn request_randomness(ctx: Context<RequestRandomness>, seed: [u8; 32]) -> Result<()> {
        let cpi_ctx = CpiContext::new(
            ctx.accounts.vrf_program.to_account_info(),
            RequestV2 {
                payer: ctx.accounts.payer.to_account_info(),
                network_state: ctx.accounts.config.to_account_info(),
                treasury: ctx.accounts.treasury.to_account_info(),
                request: ctx.accounts.request.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
            }
        );

        orao_solana_vrf::cpi::request_v2(cpi_ctx, seed)?;
        Ok(())
    }

    // pub fn use_randomness(ctx: Context<UseRandomness>) -> Result<()> {
    //     let request_account = ctx.accounts.request.load()?;
    //     let randomness = request_account.randomness.ok_or(YourError::NotFulfilled)?;
    //     // Use randomness
    //     Ok(())
    // }
    
}

pub fn get_lottery_status(ctx: Context<GetLotteryStatus>) -> Result<()> {
    Ok(())
}


#[error_code]
pub enum LotteryError {
    #[msg("La lotería no está activa")]
    LotteryNotActive,
}


#[derive(Accounts)]
pub struct StartLottery<'info> {
    #[account(init, payer = user, space = 8 + 8 + 1 + (32*10))] // Ejemplo de cómo almacenar un premio y un estado. Added space for 10 player public keys (32 bytes each)
    pub lottery: Account<'info, Lottery>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ParticipateInLottery<'info> {
    #[account(mut)]
    pub lottery: Account<'info, Lottery>,
    #[account(mut)]
    pub lottery_account: AccountInfo<'info>,
    #[account(mut)]
    pub player: Signer<'info>,
    pub system_program: Program<'info, System>,

}

#[derive(Accounts)]
pub struct DrawWinner<'info> {
    #[account(mut)]
    pub lottery: Account<'info, Lottery>,
    #[account(mut)]
    pub winner: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct RequestRandomness<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut)]
    pub config: AccountInfo<'info>,  // Network state
    #[account(mut)]
    pub treasury: AccountInfo<'info>,
    #[account(mut)]
    pub request: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub vrf_program: Program<'info, OraoVrf>,
}

#[account]
pub struct Lottery {
    pub prize_amount: u64,  
    pub is_active: bool,     
    pub players: Vec<Pubkey>, 
}

#[derive(Accounts)]
pub struct GetLotteryStatus<'info> {
    pub lottery: Account<'info, Lottery>,
}
