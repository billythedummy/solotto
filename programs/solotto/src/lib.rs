use anchor_lang::prelude::*;

const MAX_PLAYERS: u16 = 20;

/// 0.02 SOL
const TICKET_PRICE_LAMPORTS: u64 = 20_000_000;

/// Percentage of the pool the program keeps for maintenance/profit
const POOL_CUT: f64 = 0.01;

// Note: ALL methods and fns in a #[program] mod are solana instruction handlers
// and must include Context<> param, else the custom attribute will panic
// All helper fns must therefore be outside this mod
#[program]
pub mod solotto {
    use super::*;

    #[state]
    pub struct Pool {
        /// Creator of this program, the only one authorized to start and stop the game and pay out
        pub authority: Pubkey,

        /// Players currently in the pot
        pub players: [Pubkey; 20], // const expr cant be parsed by anchor idl generation

        /// How many players in `players`
        pub n_players: u16,

        /// Is there a game currently ongoing
        pub is_ongoing: bool,
    }

    impl Pool {
        pub fn new(ctx: Context<Auth>) -> Result<Self> {
            Ok(Self {
                authority: *ctx.accounts.authority.key,
                players: [Pubkey::default(); 20],
                n_players: 0,
                is_ongoing: false,
            })
        }

        /// Starts a new game
        #[access_control(is_same_account(self.authority, *ctx.accounts.authority.key))]
        pub fn start_game(&mut self, ctx: Context<Auth>) -> Result<()> {
            if self.is_ongoing {
                return Err(LottoError::GameOngoing.into());
            }
            self.is_ongoing = true;
            self.n_players = 0;
            Ok(())
        }

        /// Ends the game and pays out the lamports to one of the accounts in `players`
        #[access_control(is_same_account(self.authority, *ctx.accounts.authority.key))]
        pub fn payout(&mut self, ctx: Context<Payout>) -> Result<()> {
            if !self.is_ongoing {
                return Err(LottoError::NoGameOngoing.into());
            }
            if self.n_players == 0 {
                return Err(LottoError::NotEnoughPlayers.into());
            }
            let payout = calc_payout(self.n_players);
            let pool = ctx.accounts.state.to_account_info();
            **pool.try_borrow_mut_lamports()? -= payout;
            **ctx.accounts.winner.try_borrow_mut_lamports()? += payout;
            self.is_ongoing = false;
            self.n_players = 0;
            Ok(())
        }

        /// Buy a lottery ticket
        pub fn buy_ticket(&mut self, ctx: Context<BuyTicket>) -> Result<()> {
            if !self.is_ongoing {
                return Err(LottoError::NoGameOngoing.into());
            }
            if self.n_players == MAX_PLAYERS {
                return Err(LottoError::MaxPlayers.into());
            }
            let pool = ctx.accounts.state.to_account_info();
            self.players[self.n_players as usize] = *ctx.accounts.buyer.key;
            self.n_players += 1;
            **ctx.accounts.buyer.try_borrow_mut_lamports()? -= TICKET_PRICE_LAMPORTS;
            **pool.try_borrow_mut_lamports()? += TICKET_PRICE_LAMPORTS;
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

/// Generates a "random" number in [0, max)
/// there's no RNG available, use clock as source of entropy
/// TODO
fn rand(_max: u16) -> u16 {
    0
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
pub struct Payout<'info> {
    #[account(signer)]
    authority: AccountInfo<'info>,
    state: ProgramState<'info, Pool>,
    #[account(mut)]
    winner: AccountInfo<'info>,
    system_program: AccountInfo<'info>,
    clock: Sysvar<'info, Clock>,
    recent_block_hashes: Sysvar<'info, RecentBlockhashes>,
}

#[derive(Accounts)]
pub struct BuyTicket<'info> {
    #[account(signer, mut)]
    buyer: AccountInfo<'info>,
    state: ProgramState<'info, Pool>,
}

// More on the signer attribute since I sometimes get confused
//
// From the anchor docs:
// `signer` attr enforces that the account corresponding to this field signed the transaction
//
// Take Auth as example. Any instruction handler that has Auth as its Context is guaranteed to have
// the rpc/transaction signed by the `authority` account. So, to check identity, you can simply
// compare this AccountInfo's public key to the public key of the desired identity.
//
// In this case, we want to make sure that
// only the creator (me) can call certain functions like payout(), so we save my public
// key to the state struct upon program initialization and then compare it to
// the `authority` field that comes in from any client's rpc call to payout() via
// the access_control fn to make sure that the call was indeed signed by me.

/// ERROR STRUCTS

#[error]
pub enum LottoError {
    #[msg("Not enough players in the pool")]
    NotEnoughPlayers,

    #[msg("Game already started")]
    GameOngoing,

    #[msg("No game ongoing")]
    NoGameOngoing,

    #[msg("Max players reached")]
    MaxPlayers,
}
