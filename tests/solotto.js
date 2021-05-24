const anchor = require('@project-serum/anchor');
const sjcl = require('sjcl');
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
      programId: anchor.web3.SystemProgram.programId,
    })
  );
  await program.provider.send(tx, [keyPair]);
}

function hexToBytes(hex) {
  for (var bytes = [], c = 0; c < hex.length; c += 2)
  bytes.push(parseInt(hex.substr(c, 2), 16));
  return bytes;
}

describe('solotto', () => {

  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.Provider.env());

  const program = anchor.workspace.Solotto;
  const authorityPair = program.provider.wallet.payer;
  const authority = authorityPair.publicKey;
  const buyer = anchor.web3.Keypair.generate();
  const buyer2 = anchor.web3.Keypair.generate();

  async function startGame(winningSeed, salt) {
    const commit = hexToBytes(sjcl.codec.hex.fromBits(
      sjcl.hash.sha256.hash(winningSeed + salt)
    ));
    await program.state.rpc.startGame(commit, {
      accounts: {
        authority
      }
    });
  }

  // Initialize
  it('Initialize', async () => {
    await program.state.rpc.new({
      accounts: {
        authority
      }
    });
  });

  it('Start', async () => {
    await startGame(69, ":fsgasfdf");
  });

  it('1 person game', async () => {
    const buyerFunds = 25000000;
    await createWallet(program, buyer, buyerFunds);
    let buyerInfo = await program.provider.connection.getAccountInfo(buyer.publicKey);
    assert.ok(buyerInfo.lamports === buyerFunds);

    const stateAddress = await program.state.address();
    let pool = await program.provider.connection.getAccountInfo(stateAddress);
    const poolStarting = pool.lamports;

    await program.state.rpc.buyTicket({
      accounts: {
        buyer: buyer.publicKey,
        state: stateAddress,
        systemProg: anchor.web3.SystemProgram.programId,
      },
      signers: [buyer]
    });
    
    buyerInfo = await program.provider.connection.getAccountInfo(buyer.publicKey);
    assert.ok(buyerInfo.lamports === buyerFunds - TICKET_PRICE);

    pool = await program.provider.connection.getAccountInfo(stateAddress);
    assert.ok(pool.lamports === poolStarting + TICKET_PRICE);

    let stateStruct = await program.state();
    assert.ok(stateStruct.nPlayers === 1);
    assert.ok(stateStruct.gameState.ongoing);
    assert.ok(stateStruct.players[0].equals(buyer.publicKey));

    await program.state.rpc.endGame(69 + ":fsgasfdf", {
      accounts: {
        authority,
        clock: anchor.web3.SYSVAR_CLOCK_PUBKEY
      },
      // the call seems to have gone through without the signers array...
      signers: [authorityPair]
    });
    stateStruct = await program.state();
    assert.ok(stateStruct.gameState.completed);

    await program.state.rpc.payout({
      accounts: {
        authority,
        state: stateAddress,
        winner: buyer.publicKey,
      },
      signers: [authorityPair]
    });

    stateStruct = await program.state();
    assert.ok(stateStruct.nPlayers === 0);
    assert.ok(stateStruct.gameState.inactive);

    buyerInfo = await program.provider.connection.getAccountInfo(buyer.publicKey);
    assert.ok(buyerInfo.lamports === 24_980_000);

  });

  it('2 person game', async () => {
    const buyerFunds = 25000000;
    await createWallet(program, buyer2, buyerFunds);
    const stateAddress = await program.state.address();

    const winningSeed = 45324545675435465;
    const salt = ":fgsvgbhhgfdhffghsgafcsdvggerfeghjhhtg";
    await startGame(winningSeed, salt);

    let pool = await program.provider.connection.getAccountInfo(stateAddress);
    const poolStarting = pool.lamports;

    await program.state.rpc.buyTicket({
      accounts: {
        buyer: buyer.publicKey,
        state: stateAddress,
        systemProg: anchor.web3.SystemProgram.programId,
      },
      signers: [buyer]
    });

    await program.state.rpc.buyTicket({
      accounts: {
        buyer: buyer2.publicKey,
        state: stateAddress,
        systemProg: anchor.web3.SystemProgram.programId,
      },
      signers: [buyer2]
    });

    buyerInfo2 = await program.provider.connection.getAccountInfo(buyer2.publicKey);
    assert.ok(buyerInfo2.lamports === buyerFunds - TICKET_PRICE);

    pool = await program.provider.connection.getAccountInfo(stateAddress);
    assert.ok(pool.lamports === poolStarting + 2*TICKET_PRICE);

    let stateStruct = await program.state();
    assert.ok(stateStruct.nPlayers === 2);
    assert.ok(stateStruct.gameState.ongoing);

    await program.state.rpc.endGame(winningSeed + salt, {
      accounts: {
        authority,
        clock: anchor.web3.SYSVAR_CLOCK_PUBKEY
      },
      // the call seems to have gone through without the signers array...
      signers: [authorityPair]
    });

    stateStruct = await program.state();
    winner = stateStruct.players[0];

    await program.state.rpc.payout({
      accounts: {
        authority,
        state: stateAddress,
        winner,
      },
      signers: [authorityPair]
    });

    buyerInfo = await program.provider.connection.getAccountInfo(buyer.publicKey);
    buyerInfo2 = await program.provider.connection.getAccountInfo(buyer2.publicKey);

    console.log(buyerInfo.lamports);
    console.log(buyerInfo2.lamports);

    stateStruct = await program.state();
    assert.ok(stateStruct.nPlayers === 0);
    assert.ok(stateStruct.gameState.inactive);
  });

});
