import * as anchor from "@coral-xyz/anchor";
import { Program, AnchorError } from "@coral-xyz/anchor";
import { LotteryGame } from "../target/types/lottery_game";
import {
  Keypair,
  SystemProgram,
  LAMPORTS_PER_SOL,
  PublicKey,
  SYSVAR_CLOCK_PUBKEY,
} from "@solana/web3.js";
import { expect } from "chai";

const LOTTERY_PDA_SEED = Buffer.from("lottery");
const ENTRY_FEE = 100_000_000; 

describe("lottery-game", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.LotteryGame as Program<LotteryGame>;
  const connection = provider.connection;

  const lotteryManager = Keypair.generate();
  const player1 = Keypair.generate();
  const player2 = Keypair.generate();
  const player3 = Keypair.generate();
  const nonLotteryManager = Keypair.generate();

  const lotteryId = new anchor.BN(Math.floor(Math.random() * 1000000));
  const prizeAmount = new anchor.BN(1 * LAMPORTS_PER_SOL);
  
  let lotteryPDA: PublicKey;

  const getLotteryPDA = (auth: PublicKey, id: anchor.BN): [PublicKey, number] => {
    return anchor.web3.PublicKey.findProgramAddressSync(
      [LOTTERY_PDA_SEED, auth.toBuffer(), id.toArrayLike(Buffer, "le", 8)],
      program.programId
    );
  };

  const airdrop = async (account: Keypair, amount = 2 * LAMPORTS_PER_SOL) => {
    const airdropSig = await connection.requestAirdrop(account.publicKey, amount);
    const latestBlockhash = await connection.getLatestBlockhash();
    await connection.confirmTransaction({
      signature: airdropSig,
      blockhash: latestBlockhash.blockhash,
      lastValidBlockHeight: latestBlockhash.lastValidBlockHeight
    }, "confirmed");
  };

  before(async () => {
    await airdrop(lotteryManager, 5 * LAMPORTS_PER_SOL);
    await airdrop(player1);
    await airdrop(player2);
    await airdrop(player3);
    await airdrop(nonLotteryManager);

    [lotteryPDA] = getLotteryPDA(lotteryManager.publicKey, lotteryId);
  });

  describe("start_lottery", () => {
    it("Should initialize the lottery successfully", async () => {
      await program.methods
        .startLottery(prizeAmount, lotteryId)
        .accounts({
          lottery: lotteryPDA,
          lotteryManager: lotteryManager.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([lotteryManager])
        .rpc();

      const lotteryAccount = await program.account.lottery.fetch(lotteryPDA);
      expect(lotteryAccount.lotteryManager.toBase58()).to.equal(lotteryManager.publicKey.toBase58());
      expect(lotteryAccount.prizeAmount.toString()).to.equal(prizeAmount.toString());
      expect(lotteryAccount.isActive).to.be.true;
      expect(lotteryAccount.winner).to.be.null;
      expect(lotteryAccount.players).to.be.empty;
    });

    it("Should fail with zero prize amount", async () => {
      const zeroPrizeLotteryId = new anchor.BN(lotteryId.toNumber() + 1);
      const [zeroPrizePDA] = getLotteryPDA(lotteryManager.publicKey, zeroPrizeLotteryId);
      
      try {
        await program.methods
          .startLottery(new anchor.BN(0), zeroPrizeLotteryId)
          .accounts({ 
            lottery: zeroPrizePDA, 
            lotteryManager: lotteryManager.publicKey, 
            systemProgram: SystemProgram.programId 
          })
          .signers([lotteryManager])
          .rpc();
        expect.fail("Should have failed with zero prize");
      } catch (err) {
        expect(err).to.be.instanceOf(AnchorError);
        expect((err as AnchorError).error.errorCode.code).to.equal("ZeroPrizeAmount");
      }
    });
  });

  describe("participate_in_lottery", () => {
    it("Players should participate successfully and pay entry fee", async () => {
      // get initial balances
      const player1InitialBalance = await connection.getBalance(player1.publicKey);
      const lotteryInitialBalance = await connection.getBalance(lotteryPDA);
      const lotteryAccountBefore = await program.account.lottery.fetch(lotteryPDA);
      const totalPotBefore = lotteryAccountBefore.totalPot.toNumber();
    

      await program.methods.participateInLottery()
        .accounts({ 
          lottery: lotteryPDA, 
          player: player1.publicKey,
          systemProgram: SystemProgram.programId
        })
        .signers([player1])
        .rpc();
      
      // check player was added
      let lotteryAccount = await program.account.lottery.fetch(lotteryPDA);
      expect(lotteryAccount.players).to.have.lengthOf(1);
      expect(lotteryAccount.players[0].toBase58()).to.equal(player1.publicKey.toBase58());
      

      const player1AfterBalance = await connection.getBalance(player1.publicKey);
      const balanceDecrease = player1InitialBalance - player1AfterBalance;
      const txFeeBuffer = 10000; 
      
      // make sure plaer pays entry fee plus some tiny buffer
      expect(balanceDecrease).to.be.gte(ENTRY_FEE);
      expect(balanceDecrease).to.be.lte(ENTRY_FEE + txFeeBuffer);
      

      const lotteryAfterBalance = await connection.getBalance(lotteryPDA);
      expect(lotteryAfterBalance).to.equal(lotteryInitialBalance + ENTRY_FEE);
      
      // make sure pot was updated
      expect(lotteryAccount.totalPot.toNumber()).to.equal(totalPotBefore + ENTRY_FEE);
    
      // another player joins
      await program.methods.participateInLottery()
        .accounts({ 
          lottery: lotteryPDA, 
          player: player2.publicKey,
          systemProgram: SystemProgram.programId
        })
        .signers([player2])
        .rpc();
    
      lotteryAccount = await program.account.lottery.fetch(lotteryPDA);
      expect(lotteryAccount.players).to.have.lengthOf(2);
      // make sure pot is updated again
      expect(lotteryAccount.totalPot.toNumber()).to.equal(totalPotBefore + (2 * ENTRY_FEE));
    });

    it("Should fail if a player tries to participate twice", async () => {
      try {
        await program.methods.participateInLottery()
          .accounts({ lottery: lotteryPDA, player: player1.publicKey })
          .signers([player1])
          .rpc();
        expect.fail("Should have failed for duplicate participation");
      } catch(err) {
        expect(err).to.be.instanceOf(AnchorError);
        expect((err as AnchorError).error.errorCode.code).to.equal("AlreadyParticipating");
      }
    });
  });

  describe("draw_winner", () => {
    it("Should fail if called by non-lottery_manager", async () => {
      try {
        await program.methods
          .drawWinner()
          .accounts({
            lottery: lotteryPDA,
            lotteryManager: nonLotteryManager.publicKey,
            clock: SYSVAR_CLOCK_PUBKEY,
          })
          .signers([nonLotteryManager])
          .rpc();
        expect.fail("Should have failed due to constraint violation");
      } catch (err) {
        expect(err).to.be.instanceOf(AnchorError);
        expect((err as AnchorError).error.errorCode.code).to.equal("ConstraintSeeds");
      }
    });

    it("Authority should draw a winner successfully", async () => {
      await program.methods.drawWinner()
        .accounts({ 
          lottery: lotteryPDA, 
          lotteryManager: lotteryManager.publicKey, 
          clock: SYSVAR_CLOCK_PUBKEY 
        })
        .signers([lotteryManager])
        .rpc();
      
      const lotteryAccount = await program.account.lottery.fetch(lotteryPDA);
      expect(lotteryAccount.isActive).to.be.false;
      expect(lotteryAccount.winner).to.not.be.null;
    });
  });

  describe("claim_prize", () => {
    let winnerAccount: Keypair;
    let loserAccount: Keypair;

    before(async () => {
      const lotteryAccount = await program.account.lottery.fetch(lotteryPDA);
      const winnerPk = lotteryAccount.winner;
      
      if (winnerPk.equals(player1.publicKey)) {
        winnerAccount = player1;
        loserAccount = player2;
      } else {
        winnerAccount = player2;
        loserAccount = player1;
      }
    });

    it("Should fail if claimant is not the winner", async () => {
      try {
        await program.methods
          .claimPrize()
          .accounts({
            lottery: lotteryPDA,
            winner: loserAccount.publicKey,
          })
          .signers([loserAccount])
          .rpc();
        expect.fail("Should have failed for non-winner claim");
      } catch (err) {
        expect(err).to.be.instanceOf(AnchorError);
        expect((err as AnchorError).error.errorCode.code).to.equal("NotTheWinner");
      }
    });

    it("Winner should claim the prize successfully", async () => {
 
      const initialWinnerBalance = await connection.getBalance(winnerAccount.publicKey);
      const lotteryBalance = await connection.getBalance(lotteryPDA);
      
  
      const lotteryAccount = await program.account.lottery.fetch(lotteryPDA);
      const prizeAmount = lotteryAccount.prizeAmount.toNumber();
      

      await program.methods
        .claimPrize()
        .accounts({
          lottery: lotteryPDA,
          winner: winnerAccount.publicKey,
        })
        .signers([winnerAccount])
        .rpc();
    
      // check PDA is actually closed by making sure acct. data is null
      const lotteryAccountInfo = await connection.getAccountInfo(lotteryPDA);
      expect(lotteryAccountInfo).to.be.null;
    

      const finalWinnerBalance = await connection.getBalance(winnerAccount.publicKey);
      
      // The winner should receive the lottery balance (prize + rent)
      // minus a small transaction fee
      const expectedIncrease = lotteryBalance;
      const txFeeBuffer = 5000; // 5000 lamports buffer for transaction fee
      
      expect(finalWinnerBalance).to.be.gte(initialWinnerBalance + expectedIncrease - txFeeBuffer);
      expect(finalWinnerBalance).to.be.lte(initialWinnerBalance + expectedIncrease);
      
      // check that the actual increase is close to the lottery balance
      const actualIncrease = finalWinnerBalance - initialWinnerBalance;
      expect(actualIncrease).to.be.closeTo(lotteryBalance, txFeeBuffer);
    });
  });
});