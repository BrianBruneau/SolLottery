import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { LotteryGame } from "../target/types/lottery_game";
import { assert } from "chai";

describe("lottery_game", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.LotteryGame as Program<LotteryGame>;

  let lotteryAccount = anchor.web3.Keypair.generate();
  let player = provider.wallet;

  const prizeAmount = new anchor.BN(1_000_000); // 0.001 SOL

  it("Starts the lottery", async () => {
    await program.methods
      .startLottery(prizeAmount)
      .accounts({
        lottery: lotteryAccount.publicKey,
        user: player.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([lotteryAccount])
      .rpc();

    const lottery = await program.account.lottery.fetch(lotteryAccount.publicKey);
    assert.ok(lottery.isActive);
    assert.strictEqual(lottery.prizeAmount.toNumber(), prizeAmount.toNumber());
    console.log(" Lottery started successfully");
  });

  it("Joins the lottery", async () => {
    await program.methods
      .participateInLottery()
      .accounts({
        lottery: lotteryAccount.publicKey,
        player: player.publicKey,
      })
      .rpc();

    const lottery = await program.account.lottery.fetch(lotteryAccount.publicKey);
    assert.strictEqual(lottery.players.length, 1);
    console.log(" Player joined the lottery");
  });

  it("Draws a winner and transfers the prize", async () => {
    const winnerAccount = anchor.web3.Keypair.generate();

    //  making sure the account exists on testnet
    const sig = await provider.connection.requestAirdrop(winnerAccount.publicKey, 1_000_000);
    await provider.connection.confirmTransaction(sig);

    const balanceBefore = await provider.connection.getBalance(winnerAccount.publicKey);

    await program.methods
      .drawWinner()
      .accounts({
        lottery: lotteryAccount.publicKey,
        winner: winnerAccount.publicKey,
      })
      .rpc();

    const balanceAfter = await provider.connection.getBalance(winnerAccount.publicKey);
    const received = balanceAfter - balanceBefore;

    console.log(" Winner selected. Prize received:", received / 1e9, "SOL");
    assert.isAbove(received, 0);
  });
});
