use anchor_lang::prelude::*;
use anchor_lang::system_program; 
use orao_solana_vrf::cpi::accounts::RequestV2;
use orao_solana_vrf::program::OraoVrf;


declare_id!("5hX9mhahXK14yftY8ZePS2dZBp1m7zn7Nm61rBtyBTbf");

const MAX_PLAYERS: usize = 50;
const LOTTERY_PDA_SEED: &[u8] = b"lottery";
const ENTRY_FEE: u64 = 100_000_000; // 0.1 sol entry fee to dissuade smurf accounts

#[program]
pub mod lottery_game {
    use super::*;

    // initializes a new lottery, transferring the prize amount from the lottery_manager to the lottery PDA
    pub fn start_lottery(ctx: Context<StartLottery>, prize_amount: u64, lottery_id: u64) -> Result<()> {
        let lottery = &mut ctx.accounts.lottery;
        let lottery_manager = &ctx.accounts.lottery_manager;
        let system_program = &ctx.accounts.system_program;

        if prize_amount == 0 {
            return Err(LotteryError::ZeroPrizeAmount.into());
        }

        // transfer prize directly to the lottery PDA. The account needs rent exemption funded by the lottery_manager via `init`,
        // security note: transfers occur FROM the signer, to a newly init'd lottery PDA, so should be no risk of abusing this to steal from arbitrary accounts
        msg!("Transferring prize {} to lottery PDA {}", prize_amount, lottery.key());
        let cpi_context = CpiContext::new(
            system_program.to_account_info(),
            system_program::Transfer {
                from: lottery_manager.to_account_info(),
                to: lottery.to_account_info(), 
            },
        );
        system_program::transfer(cpi_context, prize_amount)?;
        msg!("Prize transferred to lottery account.");


        // Initialize Lottery State
        lottery.lottery_manager = lottery_manager.key();
        lottery.lottery_id = lottery_id;
        lottery.prize_amount = prize_amount;
        lottery.is_active = true;
        lottery.winner = None;
        lottery.players = Vec::with_capacity(MAX_PLAYERS);
        lottery.total_pot = prize_amount; // Initialize total pot with prize amount
        
        msg!("Lottery {} started with prize {}", lottery_id, prize_amount);
        Ok(())
    }

    // players enter lottery by being added to the lottery PDA's player array, 
    pub fn participate_in_lottery(ctx: Context<ParticipateInLottery>) -> Result<()> {
        let lottery = &mut ctx.accounts.lottery;
        let player = &ctx.accounts.player;
        let system_program = &ctx.accounts.system_program;

        // lottery must be active to participate
        if !lottery.is_active {
            return Err(LotteryError::LotteryNotActive.into());
        }
        // player cap, mainly to prevent excessive players joining
        if lottery.players.len() >= MAX_PLAYERS {
            return Err(LotteryError::MaxPlayersReached.into());
        }
        // make sure no duplicate accounts per lottery
        if lottery.players.contains(&player.key()) {
            return Err(LotteryError::AlreadyParticipating.into());
        }

        // economic disincentive against smurf/sockpuppet accounts: every new participant must pay entry fee, which is added to the pot
        let cpi_context = CpiContext::new(
            system_program.to_account_info(),  
            system_program::Transfer {        
                from: player.to_account_info(),
                to: lottery.to_account_info(),
            },
        );
        system_program::transfer(cpi_context, ENTRY_FEE)?;

        lottery.players.push(player.key());
        lottery.total_pot = lottery.total_pot.checked_add(ENTRY_FEE)
        .ok_or(LotteryError::MathOverflow)?;
        msg!("Player {} entered lottery {}", player.key(), lottery.lottery_id);
        Ok(())
    }

    // selects winner. Only callable by lottery manager. Does not actually transfer/close any accounts
    pub fn draw_winner(ctx: Context<DrawWinner>) -> Result<()> {
        let lottery = &mut ctx.accounts.lottery;

        // prevent double-drawing
        if lottery.winner.is_some() {
            return Err(LotteryError::WinnerAlreadyDrawn.into());
        }
        // lottery must be active. Inactive lottery implies winner has already been drawn
        if !lottery.is_active {
            return Err(LotteryError::LotteryNotActive.into());
        }
        // make sure lottery actually has players
        let players = &lottery.players;
        if players.is_empty() {
            return Err(LotteryError::NoPlayers.into());
        }

        // PLACEHOLDER RANDOMNESS. uses needs to be updated with solana-vrf
        let clock = Clock::get()?;
        let winner_index = clock.slot % (players.len() as u64);
        let winner_pubkey = players[winner_index as usize];
        let winner = lottery.players[0]; 
        let winner_account = &mut ctx.accounts.winner;
        **winner_account.to_account_info().lamports.borrow_mut() += lottery.prize_amount;

        // set the winner and deactivate lottery
        lottery.winner = Some(winner_pubkey);
        lottery.is_active = false; 

        msg!("Lottery {} winner drawn: {}", lottery.lottery_id, winner_pubkey);
        Ok(())
    }

    // transfers lamports from + closes account of winner via ClaimPrize close . Mostly logging
    pub fn claim_prize(ctx: Context<ClaimPrize>) -> Result<()> {
        let lottery = &mut ctx.accounts.lottery;
        let winner_signer = &ctx.accounts.winner;
    
        let lamports_being_transferred = lottery.to_account_info().lamports();
        msg!("Closing lottery account {} and transferring {} lamports to winner {}",
             lottery.key(), lamports_being_transferred, winner_signer.key());
    
        msg!("Prize claimed and lottery account closed.");
        Ok(())
    }

} 


#[account]
pub struct Lottery {
    pub lottery_manager: Pubkey,     // 32 bytes
    pub prize_amount: u64,     // 8 bytes 
    pub is_active: bool,       // 1 byte
    pub winner: Option<Pubkey>, // 1 + 32 bytes
    pub players: Vec<Pubkey>,  // 4 + (MAX_PLAYERS * 32) bytes
    pub lottery_id: u64,      // 8 bytes
    pub total_pot: u64,       // 8 bytes - Tracks total prize + entry fees
}

impl Lottery {
    const LEN: usize = 8 // discriminator
        + 32 // lottery_manager
        + 8  // prize_amount: u64
        + 1  // is_active: bool
        + 1 + 32 // winner: Option<Pubkey>
        + 4 + (MAX_PLAYERS * 32) // players: Vec<Pubkey>
        + 8  // lottery_id
        + 8; // total_pot
}

#[derive(Accounts)]
#[instruction(prize_amount: u64, lottery_id: u64)]
pub struct StartLottery<'info> {
    #[account(
        init, // new lottery PDA for each lottery
        payer = lottery_manager,
        space = Lottery::LEN,
        seeds = [LOTTERY_PDA_SEED, lottery_manager.key().as_ref(), &lottery_id.to_le_bytes()],
        bump
    )]
    pub lottery: Account<'info, Lottery>,
    #[account(mut)]
    pub lottery_manager: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ParticipateInLottery<'info> {
    #[account(
        mut,
        seeds = [LOTTERY_PDA_SEED, lottery.lottery_manager.as_ref(), &lottery.lottery_id.to_le_bytes()],
        bump
    )]
    pub lottery: Account<'info, Lottery>,
    #[account(mut)]
    pub player: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DrawWinner<'info> {
    #[account(
        mut,
        has_one = lottery_manager @ LotteryError::UnauthorizedLotteryManager,
        seeds = [LOTTERY_PDA_SEED, lottery_manager.key().as_ref(), &lottery.lottery_id.to_le_bytes()],
        bump
    )]
    pub lottery: Account<'info, Lottery>,
    pub lottery_manager: Signer<'info>,
    // remember to remove if actual VRF is implemented
    pub clock: Sysvar<'info, Clock>,
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

#[derive(Accounts)]
pub struct ClaimPrize<'info> {
    #[account(
        mut,
        seeds = [LOTTERY_PDA_SEED, lottery.lottery_manager.as_ref(), &lottery.lottery_id.to_le_bytes()],
        bump,
        // ensure lottery is inactive and thus closeable
        constraint = !lottery.is_active @ LotteryError::LotteryStillActive,
        // ensure winner is actually the drawn winner in the PDA
        constraint = lottery.winner.is_some() @ LotteryError::WinnerNotDrawn,
        // ensure signer is actually the winner
        constraint = lottery.winner == Some(winner.key()) @ LotteryError::NotTheWinner,
        // close the lottery PDA and transfer all lamports to winner
        close = winner
    )]
    pub lottery: Account<'info, Lottery>,

    #[account(mut)]
    pub winner: Signer<'info>,
}

#[error_code]
pub enum LotteryError {
    #[msg("Lottery is not active.")]
    LotteryNotActive,
    #[msg("Lottery is still active.")]
    LotteryStillActive,
    #[msg("Maximum number of players reached.")]
    MaxPlayersReached,
    #[msg("Player is already participating in the lottery.")]
    AlreadyParticipating,
    #[msg("No players have entered the lottery.")]
    NoPlayers,
    #[msg("Winner has not been drawn yet.")]
    WinnerNotDrawn,
    #[msg("Winner has already been drawn.")]
    WinnerAlreadyDrawn,
    #[msg("You are not the winner specified in the lottery.")]
    NotTheWinner,
    #[msg("Prize amount must be greater than zero.")]
    ZeroPrizeAmount,
    #[msg("Only the lottery manager can perform this action.")]
    UnauthorizedLotteryManager,
    #[msg("Math overflow occurred.")]
    MathOverflow,
}

