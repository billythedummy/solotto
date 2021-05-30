# Solotto

Lottery program on the [Solana](https://github.com/solana-labs) network. Built using [anchor](https://github.com/project-serum/anchor). For recreational and educational purposes.

NOT AFFILIATED IN ANY WAY TO [SolLotto](https://solloto.io). SIMILARITIES IN NAME ARE PURE COINCIDENCE.

# Notes for my own future reference
- All AccountInfos must be passed at run time, including CPIs. No discovering new accounts to read/write to in the middle of a instruction handler.
- The Solana runtime is more constrained than you think. Max possible account sizes are tiny. Make use of PDAs and separate accounts as much as possible. If I were to start over I would probably just hold the game state and pool balance in the state account and for each person that buys a ticket, create an empty associated account from their public key. Then just use `getProgramAccounts` on the backend to manipulate them and drain and delete all of them at the end of a game. Wouldn't be limited to 31 max players then.
- Always backup then delete accounts, or prepare them appropriately, before deploying an update that changes the struct definition. If you don't, all of your future transactions will throw `An account's data contents was invalid` and none of them will go through.
- Accounts are allocated space on creation(?). When using variable length data structures like `Option`, make sure to allocate max possible memory on creation.
- Don't forget to update the `idl.json` file in your frontend/backend whenever you update.