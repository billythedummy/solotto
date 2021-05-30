const anchor = require('@project-serum/anchor');
const arg = require('arg');
const BN = require('bn.js');
const sjcl = require('sjcl');

const INIT_CMD = "init";
const STATE_CMD = "state";
const START_CMD = "start";
const END_CMD = "end";
const PAYOUT_CMD = "payout";
const DEL_CMD = "del";

const HANDLERS = {
  [INIT_CMD]: init,
  [STATE_CMD]: state,
  [START_CMD]: start,
  [END_CMD]: end,
  [PAYOUT_CMD]: payout,
  [DEL_CMD]: del
};

function usage() {
  console.log("Usage: solotto <PROGRAM_ID> <CMD> <CMD-ARGS>");
  console.log();
  console.log("CMD list:")
  console.log(`${INIT_CMD}     Initializes the state account. Can only be called if state account doesn't already exist.`);
  console.log(`${STATE_CMD}    Prints the state account`);
  console.log(`${START_CMD}    Starts a new game. Args:`);
  console.log(`         - seed of the format '<NUMBER>:<RANDOM-SALT-STRING>'. YOU NEED THIS EXACT SAME STRING TO END THE GAME, MAKE SURE TO SAVE THIS`);
  console.log(`${END_CMD}      Ends the ongoing game. Args:`);
  console.log("         - the same seed that was passed into 'start'");
  console.log(`${PAYOUT_CMD}   Transfers the winnings to the lottery winner. Args:`);
  console.log(`         - the publicKey of the winner. This will be saved to players[0] of the state account on game end.`);
  console.log();
  console.log("Optional arguments: ");
  console.log("--cluster    solana cluster. Defaults to 'http://localhost:8899'");
  console.log("--wallet     wallet id. Defaults to '~/.config/solana/id.json'. OTHER WALLET INTEGRATIONS NOT DONE");
  console.log("--idl        path to idl.json. Defaults to anchor workspace if not specified");
}

function parseArgs(rawArgs) {
  const args = arg(
    {
      "--cluster": String,
      "--wallet": String,
      "--idl": String,
    },
    {
      argv: rawArgs.slice(2),
    }
  );
  return {
    _: args._,
    cluster: args["--cluster"] || "http://localhost:8899",
    wallet: args["--wallet"] || "~/.config/solana/id.json",
    idl: args["--idl"]
  };
}

async function init(solotto, authority) {
  console.log("Initializing state...");
  const txHash = await solotto.state.rpc.new({
    accounts: {
      authority: authority.publicKey
    }
  });
  console.log(`State initialized. Tx hash: ${txHash}`);
}

async function state(solotto) {
  console.log(await solotto.state());
}

function hexToBytes(hex) {
  for (var bytes = [], c = 0; c < hex.length; c += 2)
  bytes.push(parseInt(hex.substr(c, 2), 16));
  return bytes;
}

function verifySeedFormat(seed) {
  const MAX_SALT_LENGTH = 128;
  if (!seed) {
    console.log("No seed provided");
    return false;
  }
  const splitted = seed.split(":");
  if (splitted.length != 2) {
    console.log("Invalid seed format. Please provide the seed as '<NUMBER>:<RANDOM-SALT-STRING>'");
    return false;
  }
  const val = splitted[0];
  const salt = splitted[1];
  if (salt.length > MAX_SALT_LENGTH) {
    console.log(`Please pick a salt of length ${MAX_SALT_LENGTH} or less`);
    return false;
  }
  try {
    new BN(val, 10);
    return true;
  } catch (BNError) {
    console.log(`Invalid number '${val}': ${BNError}`);
    return false;
  }
}

async function start(solotto, authority, seed) {
  if (!verifySeedFormat(seed)) {
    return;
  }

  console.log("Starting game...");
  const commit = hexToBytes(sjcl.codec.hex.fromBits(
    sjcl.hash.sha256.hash(seed)
  ));

  const txHash = await solotto.state.rpc.startGame(commit, {
    accounts: {
      authority: authority.publicKey
    }
  });
  console.log(`Game started. Tx hash: ${txHash}`);
}

async function end(solotto, authority, seed) {
  if (!verifySeedFormat(seed)) {
    return;
  }

  console.log("Ending game...");
  const txHash = await solotto.state.rpc.endGame(seed, {
    accounts: {
      authority: authority.publicKey,
      clock: anchor.web3.SYSVAR_CLOCK_PUBKEY
    }
  });
  console.log(`Game Ended. Tx hash: ${txHash}`);
}

async function payout(solotto, authority) {
  const state = await solotto.state();
  if (!state.gameState.completed) {
    console.log("Game not ended");
    return;
  }
  const winnerPubkey = state.players[0];
  console.log("Paying out...");
  const stateAddress = await solotto.state.address();
  const txHash = await solotto.state.rpc.payout({
    accounts: {
      authority: authority.publicKey,
      winner: winnerPubkey,
      state: stateAddress
    }
  });
  console.log(`Winner paid. Tx hash: ${txHash}`);
}

async function del(solotto, authority) {
  console.log("Deleting pool...");
  const txHash = await solotto.state.rpc.del({
    accounts: {
      authority: authority.publicKey,
      state: await solotto.state.address(),
    }
  });
  console.log(`Pool deleted. Tx hash: ${txHash}`);
}

export async function cli(rawArgs) {
    const args = parseArgs(rawArgs);
    let positional = args._;
    if (positional.length < 2) {
      usage();
      console.log(`Insufficient args: ${positional.length}, expected at least 2`);
      return;
    }
    const programIdStr = positional[0];
    const cmd = positional[1];
    if (!(cmd in HANDLERS)) {
      usage();
      console.log(`Unrecognized cmd '${cmd}'`)
      return;
    }
    const cmdArgs = positional.slice(2);

    // Set up anchor env and program
    // TODO: change this to adapt depending on --wallet arg
    process.env.ANCHOR_PROVIDER_URL = args.cluster;
    anchor.setProvider(anchor.Provider.env());
    let idl;
    if (args.idl) {
      idl = JSON.parse(require('fs').readFileSync(args.idl, 'utf8'));
    } else {
      if ("Solotto" in anchor.workspace) {
        idl = anchor.workspace.Solotto.idl;
      } else {
        console.log("Anchor workspace not found, please specify path to idl manually");
        return;
      }
    }
    const programId = new anchor.web3.PublicKey(programIdStr);
    const solotto = new anchor.Program(idl, programId);
    const authority = solotto.provider.wallet.payer;

    await HANDLERS[cmd](solotto, authority, ...cmdArgs);
}