use anchor_lang::prelude::*;
use solana_program::hash::hash;
use solana_program::program::invoke;
use solana_program::system_instruction::transfer;

const MAX_PLAYERS: u16 = 32;

/// 0.02 SOL
const TICKET_PRICE_LAMPORTS: u64 = 20_000_000;

/// Percentage of the pool the program keeps for maintenance/profit
const POOL_CUT: f64 = 0.001;

const SALT_DELIM: &str = ":";

// Note: ALL methods and fns in a #[program] mod are solana instruction handlers
// and must include Context<> param, else the custom attribute will panic
// All helper fns must therefore be outside this mod
#[program]
pub mod solotto {
    use super::*;

    #[state]
    pub struct Pool {
        /// Which state is the game in
        pub game_state: GameState,

        /// How many players in `players`
        pub n_players: u16,

        /// Committed hash of the winner's seed
        // anchor's JS IDL can't seem to handle the Hash Type, so just store it as bytes
        pub commit: [u8; 32],

        /// Creator of this program, the only one authorized to start and stop the game and pay out
        pub authority: Pubkey,

        /// Players currently in the pot
        pub players: [Pubkey; 32], // const expr cant be parsed by anchor idl generation
    }

    impl Pool {
        pub fn new(ctx: Context<Auth>) -> Result<Self> {
            Ok(Self {
                game_state: GameState::Inactive,
                n_players: 0,
                commit: [0; 32],
                authority: *ctx.accounts.authority.key,
                players: [Pubkey::default(); MAX_PLAYERS as usize],
            })
        }

        /// Starts a new game
        #[access_control(is_same_account(self.authority, *ctx.accounts.authority.key))]
        pub fn start_game(&mut self, ctx: Context<Auth>, commit: [u8; 32]) -> Result<()> {
            if self.game_state != GameState::Inactive {
                return Err(LottoError::GameOngoing.into());
            }
            self.commit = commit;
            self.game_state = GameState::Ongoing;
            self.n_players = 0;
            Ok(())
        }

        #[access_control(is_same_account(self.authority, *ctx.accounts.authority.key))]
        pub fn end_game(&mut self, ctx: Context<EndGame>, seed_gen: String) -> Result<()> {
            if self.game_state != GameState::Ongoing {
                return Err(LottoError::NoGameOngoing.into());
            }
            if hash(seed_gen.as_ref()).to_bytes() != self.commit {
                return Err(LottoError::WrongWinningSeed.into());
            }
            if self.n_players == 0 {
                // no need for payout
                self.game_state = GameState::Inactive;
                return Ok(());
            }
            let mut split = seed_gen.split(SALT_DELIM);
            let s = match split.next() {
                Some(s) => s,
                None => return Err(LottoError::WrongWinningSeed.into()),
            };
            let winning_seed: u64 = s.parse()?;
            let winning_index =
                (winning_seed ^ ctx.accounts.clock.unix_timestamp as u64) % (self.n_players as u64);
            // set index 0 or self.players to the winner's pubkey
            self.players[0] = self.players[winning_index as usize];
            self.game_state = GameState::Completed;
            Ok(())
        }

        /// Ends the game and pays out the lamports to one of the accounts in `players`
        #[access_control(is_same_account(self.authority, *ctx.accounts.authority.key))]
        pub fn payout(&mut self, ctx: Context<Payout>) -> Result<()> {
            if self.game_state != GameState::Completed {
                return Err(LottoError::NoGameOngoing.into());
            }
            if *ctx.accounts.winner.key != self.players[0] {
                return Err(LottoError::WrongWinner.into());
            }
            let payout = calc_payout(self.n_players);
            let pool = ctx.accounts.state.to_account_info();
            **pool.try_borrow_mut_lamports()? -= payout;
            **ctx.accounts.winner.try_borrow_mut_lamports()? += payout;

            self.game_state = GameState::Inactive;
            self.n_players = 0;
            Ok(())
        }

        /// Buy a lottery ticket
        pub fn buy_ticket(&mut self, ctx: Context<BuyTicket>) -> Result<()> {
            if self.game_state != GameState::Ongoing {
                return Err(LottoError::NoGameOngoing.into());
            }
            if self.n_players == MAX_PLAYERS {
                return Err(LottoError::MaxPlayers.into());
            }
            let pool = ctx.accounts.state.to_account_info();
            self.players[self.n_players as usize] = *ctx.accounts.buyer.key;
            self.n_players += 1;
            // have to do a CPI to SystemProgram because buyer is not owned by program
            let tx = transfer(ctx.accounts.buyer.key, pool.key, TICKET_PRICE_LAMPORTS);
            invoke(
                &tx,
                &[
                    ctx.accounts.buyer.clone(),
                    pool.clone(),
                    ctx.accounts.system_prog.clone(),
                ],
            )?;
            Ok(())
        }
    }
}

fn is_same_account(k1: Pubkey, k2: Pubkey) -> Result<()> {
    if k1 != k2 {
        return Err(ProgramError::MissingRequiredSignature.into());
    }
    Ok(())
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq)]
pub enum GameState {
    Inactive,
    Ongoing,
    /// winner has been determined but not yet paid out
    Completed,
}

/// Calculates the amount to be paid out to the winner in lamports
fn calc_payout(n_players: u16) -> u64 {
    let collected = n_players as u64 * TICKET_PRICE_LAMPORTS;
    let payout = (1.0 - POOL_CUT) * (collected as f64);
    payout as u64
}

/// CONTEXT STRUCTS
/// These are the structs that are deserialized from client rpc calls
/// In solana's programming model, you must declare all accounts your instruction
/// will use when calling it. This means the only AccountInfo structs your instruction handler
/// will have access to are the ones declared in these structs.

#[derive(Accounts)]
pub struct Auth<'info> {
    #[account(signer)]
    authority: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct EndGame<'info> {
    #[account(signer)]
    authority: AccountInfo<'info>,
    clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct Payout<'info> {
    #[account(signer)]
    authority: AccountInfo<'info>,
    state: ProgramState<'info, Pool>,
    #[account(mut)]
    winner: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct BuyTicket<'info> {
    #[account(signer, mut)]
    buyer: AccountInfo<'info>,
    state: ProgramState<'info, Pool>,
    system_prog: AccountInfo<'info>,
}

/// ERROR STRUCTS

#[error]
pub enum LottoError {
    #[msg("Game already started")]
    GameOngoing,

    #[msg("No game ongoing")]
    NoGameOngoing,

    #[msg("Max players reached")]
    MaxPlayers,

    #[msg("Winning seed is different from the one commited")]
    WrongWinningSeed,

    #[msg("Payout account is not the determined winner")]
    WrongWinner,
}

impl From<core::num::ParseIntError> for Error {
    fn from(_err: core::num::ParseIntError) -> Self {
        LottoError::WrongWinningSeed.into()
    }
}
