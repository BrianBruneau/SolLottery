use anchor_lang::prelude::*;
use anchor_lang::system_program;
// Import ORAO VRF specific items
use orao_solana_vrf::{self, program::OraoVrf, state::RandomnessAccountData, CONFIG_ACCOUNT_SEED, RANDOMNESS_ACCOUNT_SEED}; // Added imports for VRF
use std::convert::TryInto; // Needed for slice conversion

declare_id!("5hX9mhahXK14yftY8ZePS2dZBp1m7zn7Nm61rBtyBTbf"); // Keep your original program ID

const MAX_PLAYERS: usize = 50;
const LOTTERY_PDA_SEED: &[u8] = b"lottery";
const ENTRY_FEE: u64 = 100_000_000; // 0.1 sol entry fee

#[program]
pub mod lottery_game {
    use super::*;

    // initializes a new lottery, transferring the prize amount from the lottery_manager to the lottery PDA
    // (No changes needed in this function)
    pub fn start_lottery(ctx: Context<StartLottery>, prize_amount: u64, lottery_id: u64) -> Result<()> {
        let lottery = &mut ctx.accounts.lottery;
        let lottery_manager = &ctx.accounts.lottery_manager;
        let system_program = &ctx.accounts.system_program;

        if prize_amount == 0 {
            return Err(LotteryError::ZeroPrizeAmount.into());
        }

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
        lottery.vrf_seed = [0u8; 32]; // Initialize VRF seed
        lottery.randomness_requested = false; // Initialize VRF request flag

        msg!("Lottery {} started with prize {}", lottery_id, prize_amount);
        Ok(())
    }

    // players enter lottery by being added to the lottery PDA's player array
    // (Added check to prevent participation if randomness already requested)
    pub fn participate_in_lottery(ctx: Context<ParticipateInLottery>) -> Result<()> {
        let lottery = &mut ctx.accounts.lottery;
        let player = &ctx.accounts.player;
        let system_program = &ctx.accounts.system_program;

        // lottery must be active to participate
        if !lottery.is_active {
            return Err(LotteryError::LotteryNotActive.into());
        }
        // prevent participation if winner selection process has started
        if lottery.randomness_requested {
             return Err(LotteryError::RandomnessAlreadyRequested.into());
        }
        // player cap
        if lottery.players.len() >= MAX_PLAYERS {
            return Err(LotteryError::MaxPlayersReached.into());
        }
        // make sure no duplicate accounts per lottery
        if lottery.players.contains(&player.key()) {
            return Err(LotteryError::AlreadyParticipating.into());
        }

        // entry fee transfer
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

    // *** NEW INSTRUCTION: Request Randomness from ORAO VRF ***
    // Only callable by the lottery manager. Initiates the VRF request.
    pub fn request_randomness(ctx: Context<RequestRandomness>, seed: [u8; 32]) -> Result<()> {
        let lottery = &mut ctx.accounts.lottery;

        // --- Pre-checks ---
        // prevent double-requesting
        if lottery.randomness_requested {
            return Err(LotteryError::RandomnessAlreadyRequested.into());
        }
        // lottery must be active
        if !lottery.is_active {
            return Err(LotteryError::LotteryNotActive.into());
        }
        // make sure lottery actually has players
        if lottery.players.is_empty() {
            return Err(LotteryError::NoPlayers.into());
        }
        // winner shouldn't be drawn yet
        if lottery.winner.is_some() {
            return Err(LotteryError::WinnerAlreadyDrawn.into());
        }

        // --- CPI Call to ORAO VRF RequestV2 ---
        msg!("Requesting randomness from ORAO VRF with seed: {:?}", seed);
        let cpi_program = ctx.accounts.vrf.to_account_info();
        let cpi_accounts = orao_solana_vrf::cpi::accounts::RequestV2 {
            payer: ctx.accounts.lottery_manager.to_account_info(), // Manager pays for VRF request
            network_state: ctx.accounts.config.to_account_info(),
            treasury: ctx.accounts.treasury.to_account_info(),
            request: ctx.accounts.random.to_account_info(), // The PDA derived from the seed
            system_program: ctx.accounts.system_program.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        // The 'force' parameter in the CPI call is the seed used for the randomness PDA
        orao_solana_vrf::cpi::request_v2(cpi_ctx, seed)?;

        // --- Update Lottery State ---
        lottery.vrf_seed = seed; // Store the seed used for this request
        lottery.randomness_requested = true; // Mark that randomness has been requested

        msg!("Randomness request sent for lottery {}. Waiting for fulfillment.", lottery.lottery_id);
        Ok(())
    }


    // *** MODIFIED INSTRUCTION: Fulfill Randomness and Draw Winner ***
    // Reads fulfilled randomness from ORAO, selects winner. Only callable by lottery manager.
    pub fn fulfill_and_draw_winner(ctx: Context<FulfillAndDrawWinner>) -> Result<()> {
        let lottery = &mut ctx.accounts.lottery;
        let randomness_account_info = &ctx.accounts.randomness;

        // --- Pre-checks ---
         if lottery.winner.is_some() {
            return Err(LotteryError::WinnerAlreadyDrawn.into());
        }
        // Lottery must be active (will be deactivated after drawing)
        if !lottery.is_active {
            return Err(LotteryError::LotteryNotActive.into());
        }
        // Randomness must have been requested
        if !lottery.randomness_requested {
             return Err(LotteryError::RandomnessNotRequested.into());
        }
         // make sure lottery actually has players (should be guaranteed by request_randomness checks, but good practice)
        let players = &lottery.players;
        if players.is_empty() {
            return Err(LotteryError::NoPlayers.into());
        }

        // --- Read ORAO Randomness Account ---
        msg!("Attempting to read randomness account {}", randomness_account_info.key());
        let account_data = randomness_account_info.try_borrow_data()?;
        // Use RandomnessAccountData::try_deserialize as per ORAO v0.4.0+ docs
        let randomness_data = RandomnessAccountData::try_deserialize(&mut &account_data[..])?;

        // Check if randomness is fulfilled
        let fulfilled_randomness = randomness_data.fulfilled_randomness()
            .ok_or(LotteryError::RandomnessNotFulfilled)?; // Error if not fulfilled

        // --- Determine Winner using VRF Randomness ---
        msg!("Randomness fulfilled: {:?}", fulfilled_randomness);
        // Use the first 8 bytes of the 64-byte randomness for the winner index calculation
        let randomness_bytes: [u8; 8] = fulfilled_randomness[0..8].try_into()
            .map_err(|_| LotteryError::InternalError)?; // Error on slice conversion failure
        let random_value = u64::from_le_bytes(randomness_bytes);
        let winner_index = random_value % (players.len() as u64);

        let winner_pubkey = players[winner_index as usize];

        // --- Update Lottery State ---
        lottery.winner = Some(winner_pubkey);
        lottery.is_active = false; // Deactivate lottery after drawing winner
        // Optional: Clear VRF state if needed, or keep for auditing
        // lottery.randomness_requested = false;
        // lottery.vrf_seed = [0u8; 32];

        msg!("Lottery {} winner drawn using ORAO VRF: {}", lottery.lottery_id, winner_pubkey);
        Ok(())
    }

    // claim_prize function remains unchanged
    pub fn claim_prize(ctx: Context<ClaimPrize>) -> Result<()> {
        let lottery = &ctx.accounts.lottery; // No mut needed here as it's closed
        let winner_signer = &ctx.accounts.winner;

        let lamports_being_transferred = lottery.to_account_info().lamports();
        msg!("Closing lottery account {} and transferring {} lamports to winner {}",
             lottery.key(), lamports_being_transferred, winner_signer.key());

        // Account closure and rent transfer happen automatically via `close = winner` constraint
        msg!("Prize claimed and lottery account closed.");
        Ok(())
    }

}

#[account]
pub struct Lottery {
    pub lottery_manager: Pubkey,     // 32 bytes
    pub prize_amount: u64,           // 8 bytes
    pub is_active: bool,             // 1 byte
    pub winner: Option<Pubkey>,      // 1 + 32 bytes
    pub players: Vec<Pubkey>,        // 4 + (MAX_PLAYERS * 32) bytes
    pub lottery_id: u64,             // 8 bytes
    pub total_pot: u64,              // 8 bytes - Tracks total prize + entry fees
    // --- Added for ORAO VRF ---
    pub vrf_seed: [u8; 32],          // 32 bytes - Seed used for the VRF request
    pub randomness_requested: bool,  // 1 byte - Flag indicating if VRF request is pending
}

impl Lottery {
    // Update the length calculation
    const LEN: usize = 8 // discriminator
        + 32 // lottery_manager
        + 8  // prize_amount
        + 1  // is_active
        + 1 + 32 // winner: Option<Pubkey>
        + 4 + (MAX_PLAYERS * 32) // players: Vec<Pubkey>
        + 8  // lottery_id
        + 8  // total_pot
        + 32 // vrf_seed <-- Added
        + 1; // randomness_requested <-- Added
}

// --- Context Structs ---

// StartLottery remains the same
#[derive(Accounts)]
#[instruction(prize_amount: u64, lottery_id: u64)]
pub struct StartLottery<'info> {
    #[account(
        init,
        payer = lottery_manager,
        space = Lottery::LEN, // Use updated LEN
        seeds = [LOTTERY_PDA_SEED, lottery_manager.key().as_ref(), &lottery_id.to_le_bytes()],
        bump
    )]
    pub lottery: Account<'info, Lottery>,
    #[account(mut)]
    pub lottery_manager: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// ParticipateInLottery remains the same
#[derive(Accounts)]
pub struct ParticipateInLottery<'info> {
    #[account(
        mut,
        seeds = [LOTTERY_PDA_SEED, lottery.lottery_manager.as_ref(), &lottery.lottery_id.to_le_bytes()],
        bump,
        // Add constraint: Ensure randomness has not been requested yet
        constraint = !lottery.randomness_requested @ LotteryError::RandomnessAlreadyRequested,
    )]
    pub lottery: Account<'info, Lottery>,
    #[account(mut)]
    pub player: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// *** NEW CONTEXT for Requesting Randomness ***
#[derive(Accounts)]
#[instruction(seed: [u8; 32])] // Seed is passed as instruction data
pub struct RequestRandomness<'info> {
    #[account(
        mut,
        has_one = lottery_manager @ LotteryError::UnauthorizedLotteryManager,
        seeds = [LOTTERY_PDA_SEED, lottery_manager.key().as_ref(), &lottery.lottery_id.to_le_bytes()],
        bump,
        // Add constraints for requesting randomness
        constraint = lottery.is_active @ LotteryError::LotteryNotActive,
        constraint = !lottery.randomness_requested @ LotteryError::RandomnessAlreadyRequested,
        constraint = lottery.winner.is_none() @ LotteryError::WinnerAlreadyDrawn,
        constraint = !lottery.players.is_empty() @ LotteryError::NoPlayers,
    )]
    pub lottery: Account<'info, Lottery>,

    #[account(mut)] // Manager signs and pays for the VRF request tx
    pub lottery_manager: Signer<'info>,

    // --- ORAO VRF Accounts ---
    /// CHECK: ORAO VRF Treasury Account
    #[account(mut)]
    pub treasury: AccountInfo<'info>,

    /// CHECK: The account receiving the randomness, derived from the seed.
    /// Needs mut because RequestV2 initializes it if needed.
    #[account(
        mut,
        seeds = [RANDOMNESS_ACCOUNT_SEED.as_ref(), seed.as_ref()], // Seed comes from instruction arg
        bump,
        seeds::program = orao_solana_vrf::ID
    )]
    pub random: AccountInfo<'info>,

    #[account(
        mut, // network state is mutable as fee is charged
        seeds = [CONFIG_ACCOUNT_SEED.as_ref()],
        bump,
        seeds::program = orao_solana_vrf::ID
    )]
    pub config: Account<'info, orao_solana_vrf::state::NetworkState>, // Use NetworkState type

    #[account(address = orao_solana_vrf::ID)] // Specify the ORAO VRF program address
    pub vrf: Program<'info, OraoVrf>,

    pub system_program: Program<'info, System>,
}


// *** MODIFIED CONTEXT for Fulfilling Randomness and Drawing Winner ***
#[derive(Accounts)]
pub struct FulfillAndDrawWinner<'info> {
    #[account(
        mut,
        has_one = lottery_manager @ LotteryError::UnauthorizedLotteryManager,
        seeds = [LOTTERY_PDA_SEED, lottery_manager.key().as_ref(), &lottery.lottery_id.to_le_bytes()],
        bump,
        // Add constraints for fulfillment
        constraint = lottery.is_active @ LotteryError::LotteryNotActive, // Must be active before drawing
        constraint = lottery.randomness_requested @ LotteryError::RandomnessNotRequested, // Must have been requested
        constraint = lottery.winner.is_none() @ LotteryError::WinnerAlreadyDrawn, // Winner shouldn't exist yet
    )]
    pub lottery: Account<'info, Lottery>,

    // Signer is still the lottery manager
    pub lottery_manager: Signer<'info>,

    // --- ORAO VRF Account to Read ---
    /// CHECK: The account holding the fulfilled randomness. Address derived from the stored seed.
    /// Not mutable as we are only reading from it here.
    #[account(
        seeds = [RANDOMNESS_ACCOUNT_SEED.as_ref(), lottery.vrf_seed.as_ref()], // Use seed stored in lottery PDA
        bump,
        seeds::program = orao_solana_vrf::ID
    )]
    pub randomness: AccountInfo<'info>,

    // No need for Clock sysvar anymore
    // No need for other ORAO accounts like config, treasury, vrf program here, just reading the result.
}


// ClaimPrize context remains the same
#[derive(Accounts)]
pub struct ClaimPrize<'info> {
    #[account(
        mut,
        seeds = [LOTTERY_PDA_SEED, lottery.lottery_manager.as_ref(), &lottery.lottery_id.to_le_bytes()],
        bump,
        // ensure lottery is inactive (winner drawn)
        constraint = !lottery.is_active @ LotteryError::LotteryStillActive,
        // ensure winner exists
        constraint = lottery.winner.is_some() @ LotteryError::WinnerNotDrawn,
        // ensure signer is the winner
        constraint = lottery.winner == Some(winner.key()) @ LotteryError::NotTheWinner,
        // close the lottery PDA and transfer lamports to winner
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
    // --- Added Errors for VRF ---
    #[msg("Randomness has already been requested for this lottery.")]
    RandomnessAlreadyRequested,
    #[msg("Randomness has not been requested for this lottery yet.")]
    RandomnessNotRequested,
    #[msg("ORAO VRF randomness for this request has not been fulfilled yet.")]
    RandomnessNotFulfilled,
    #[msg("Internal error occurred during randomness processing.")]
    InternalError,
}