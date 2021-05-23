const anchor = require('@project-serum/anchor');
const assert = require("assert");

const TICKET_PRICE = 20_000_000;

// If rpc structure does not match expected, test fn will just fail
// with unmeaningful errmsg "promise rejected with no or falsy reason"
// tho if you see this without any other log output,
// then its usually an issue in the js test code rather than the program
// e.g. passing a NodeWallet struct as a signer instead of a Keypair
// If the program fails, it will output stdout error logs "Transaction simulation failed..."

// This is how to create a wallet account for testing.
// Transfer lamports from the (infinite) program.provider.wallet
async function createWallet(program, keyPair, lamports) {
  const tx = new anchor.web3.Transaction();
  
  tx.add(
    anchor.web3.SystemProgram.createAccount({
      fromPubkey: program.provider.wallet.publicKey,
      newAccountPubkey: keyPair.publicKey,
      space: 0,
      lamports,
      programId: program.programId,
    })
  );
  await program.provider.send(tx, [keyPair]);
}

describe('solotto', () => {

  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.Provider.env());

  const program = anchor.workspace.Solotto;
  const authority_pair = program.provider.wallet.payer;
  const authority = authority_pair.publicKey;
  const buyer = anchor.web3.Keypair.generate();

  // Initialize
  it('Initialize', async () => {
    await program.state.rpc.new({
      accounts: {
        authority
      }
    });
  });

  it('Start', async () => {
    await program.state.rpc.startGame({
      accounts: {
        authority
      }
    });
  });

  it('1 person game', async () => {
    const buyer_funds = 25000000;
    await createWallet(program, buyer, buyer_funds);
    let buyer_info = await program.provider.connection.getAccountInfo(buyer.publicKey);
    assert.ok(buyer_info.lamports === buyer_funds);

    const state_address = await program.state.address();
    let pool = await program.provider.connection.getAccountInfo(state_address);
    const pool_starting = pool.lamports;

    await program.state.rpc.buyTicket({
      accounts: {
        buyer: buyer.publicKey,
        state: state_address,
      },
      signers: [buyer]
    })

    buyer_info = await program.provider.connection.getAccountInfo(buyer.publicKey);
    assert.ok(buyer_info.lamports === buyer_funds - TICKET_PRICE);

    pool = await program.provider.connection.getAccountInfo(state_address);
    assert.ok(pool.lamports === pool_starting + TICKET_PRICE);

    let state_struct = await program.state();
    assert.ok(state_struct.nPlayers === 1);
    assert.ok(state_struct.isOngoing === true);
    assert.ok(state_struct.players[0].equals(buyer.publicKey));

    await program.state.rpc.payout({
      accounts: {
        authority,
        state: state_address,
        systemProgram: anchor.web3.SystemProgram.programId,
        clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
        recentBlockHashes: anchor.web3.SYSVAR_RECENT_BLOCKHASHES_PUBKEY,
        winner: buyer.publicKey,
      },
      signers: [authority_pair]
    })

    state_struct = await program.state();
    assert.ok(state_struct.nPlayers === 0);
    assert.ok(state_struct.isOngoing === false);

    buyer_info = await program.provider.connection.getAccountInfo(buyer.publicKey);
    assert.ok(buyer_info.lamports === 24_800_000);

  });
});
