
use anchor_lang::prelude::*;


declare_id!("5hX9mhahXK14yftY8ZePS2dZBp1m7zn7Nm61rBtyBTbf");

#[program]
mod lottery_game {
    use super::*;

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
        lottery.players.push(player.key());

        Ok(())
    }

    // choosing the winner
    pub fn draw_winner(ctx: Context<DrawWinner>) -> Result<()> {
        let lottery = &mut ctx.accounts.lottery;

        if !lottery.is_active {
            return Err(LotteryError::LotteryNotActive.into());
        }

        
        let winner = lottery.players[0]; 

        let winner_account = &mut ctx.accounts.winner;
        **winner_account.to_account_info().lamports.borrow_mut() += lottery.prize_amount;


        lottery.is_active = false;

        Ok(())
    }
}

#[error_code]
pub enum LotteryError {
    #[msg("La lotería no está activa")]
    LotteryNotActive,
}


#[derive(Accounts)]
pub struct StartLottery<'info> {
    #[account(init, payer = user, space = 8 + 8 + 1)] // Ejemplo de cómo almacenar un premio y un estado
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
    pub player: Signer<'info>,
}

#[derive(Accounts)]
pub struct DrawWinner<'info> {
    #[account(mut)]
    pub lottery: Account<'info, Lottery>,
    #[account(mut)]
    pub winner: AccountInfo<'info>,
}


#[account]
pub struct Lottery {
    pub prize_amount: u64,  
    pub is_active: bool,     
    pub players: Vec<Pubkey>, 
}
