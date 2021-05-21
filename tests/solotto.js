const anchor = require('@project-serum/anchor');

// If rpc structure does not match expected, test fn will just fail
// with unmeaningful errmsg "promise rejected with no or falsy reason"

// This is how to create a wallet account for testing.
// Transfer lamports from the (infinite) program.provider.wallet
async function createWallet(program, keyPair) {
  const tx = new anchor.web3.Transaction();
  
  tx.add(
    anchor.web3.SystemProgram.createAccount({
      fromPubkey: program.provider.wallet.publicKey,
      newAccountPubkey: keyPair.publicKey,
      space: 0,
      lamports: 25000000,
      programId: program.programId,
    })
  );
  await program.provider.send(tx, [keyPair]);
}

describe('solotto', () => {

  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.Provider.env());

  const program = anchor.workspace.Solotto;
  const authority = program.provider.wallet.publicKey;
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
    await createWallet(program, buyer);
    let buyer_info = await program.provider.connection.getAccountInfo(buyer.publicKey);
    console.log(buyer_info.lamports);

    const state_address = await program.state.address();
    await program.state.rpc.buyTicket({
      accounts: {
        buyer: buyer.publicKey,
        state: state_address,
      },
      signers: [buyer]
    });

    buyer_info = await program.provider.connection.getAccountInfo(buyer.publicKey);
    console.log(buyer_info.lamports);
    state = await program.provider.connection.getAccountInfo(state_address);
    console.log(await program.state());
    console.log(state.lamports);
  });
});
