use anchor_lang::prelude::*;
use solana_program::system_instruction::transfer;

const MAX_PLAYERS: u16 = 1000;

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

    #[state(zero_copy)]
    pub struct Pool {
        /// Creator of this program, the only one authorized to stop the game and pay out
        pub authority: Pubkey,

        /// Players currently in the pot
        pub players: [Pubkey; 1000], // const expr cant be parsed by anchor idl generation

        /// How many players in `players`
        pub n_players: u16,

        /// Is there a game currently ongoing
        pub is_ongoing: bool,
    }

    impl Pool {
        pub fn new(&mut self, ctx: Context<Auth>) -> Result<()> {
            self.authority = *ctx.accounts.authority.key;
            self.n_players = 0;
            self.is_ongoing = false;
            Ok(())
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
            let winner = self.players[rand(self.n_players) as usize];
            transfer(ctx.program_id, &winner, calc_payout(self.n_players));
            self.is_ongoing = false;
            Ok(())
        }
        
        /// Buy a lottery ticket
        pub fn buy_ticket(&mut self, ctx: Context<BuyTicket>) -> Result<()> {
            if !self.is_ongoing {
                return Err(LottoError::NoGameOngoing.into());
            }
            if self.n_players == MAX_PLAYERS {
                // originally wanted to stop the game and start a new one here
                // but haven't figured out how to CPI the program itself using
                // either the state account or the program itself as authority 
                return Err(LottoError::MaxPlayers.into());
            }
            transfer(ctx.accounts.buyer.key, ctx.program_id, TICKET_PRICE_LAMPORTS);
            self.n_players += 1;
            Ok(())
        }
    }
}

/*
fn is_auth_or_pool(auth: Pubkey, ctx: &Context<Auth>) -> Result<()> {
    let caller = ctx.accounts.authority.key;
    if let Ok(()) = is_self(*caller, ctx) {
        return Ok(());
    }
    is_same_account(auth, *caller)
}

fn is_self(k: Pubkey, ctx: &Context<Auth>) -> Result<()> {
    is_same_account(k, ProgramState::address(ctx.program_id))
}
*/

fn is_same_account(k1: Pubkey, k2: Pubkey) -> Result<()> {
    if k1 != k2 {
        return Err(ProgramError::MissingRequiredSignature.into());
    }
    Ok(())
}

/// Generates a "random" number in [0, max)
/// there's no RNG available, use clock as source of entropy
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
    clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct BuyTicket<'info> {
    #[account(signer)]
    buyer: AccountInfo<'info>,
}

// More on the signer attribute since I sometimes get confused
//
// From the anchor docs:
// `signer` attr enforces that the account corresponding to this field signed the transaction
//
// Any instruction handler that has this struct as its Context is guaranteed to have
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