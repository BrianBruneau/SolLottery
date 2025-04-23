
// Handles Solana blockchain interactions for the lottery game

// connection state variables
let solanaConnection = null;
let walletAddress = null;
let lotteryProgramId = '5hX9mhahXK14yftY8ZePS2dZBp1m7zn7Nm61rBtyBTbf'; // From Anchor.toml
let lotteryAccountPubkey = 'simulated_lottery_account_for_testing'; // Simulated lottery account for testing
const ADMIN_WALLET_ADDRESSES = [
    'JE2M9NK8uQHCAw3WgDma6ijhZEP7gaaBsRGjeChTtPjt' // Admin wallet address
]; // List of admin wallet addresses


// init solana connection
async function initializeSolanaConnection() {
    try {
        solanaConnection = new solanaWeb3.Connection(
            solanaWeb3.clusterApiUrl('devnet'), // Connecting to devnet
            'confirmed'
        );
        console.log('Connected to Solana network');
        return true;
    } catch (error) {
        console.error('Failed to connect to Solana network:', error);
        displayError('Failed to connect to Solana network. Check console for details.');
        return false;
    }
}

// Connect to Phantom Wallet TODO: add more wallet connect options
async function connectWallet() {
    try {
        // Check if Phantom is installed
        const { solana } = window;
        
        if (!solana) {
            throw new Error('Phantom wallet not found! Please install Phantom wallet extension.');
        }
        
        // Request connection to the wallet
        const response = await solana.connect();
        walletAddress = response.publicKey.toString();
        
        console.log('Connected to wallet:', walletAddress);
        return walletAddress;
    } catch (error) {
        console.error('Wallet connection error:', error);
        displayError('Failed to connect wallet: ' + error.message);
        return null;
    }
}

// Disconnect from Phantom Wallet
async function disconnectWallet() {
    try {
        const { solana } = window;
        
        if (solana && solana.isConnected) {
            await solana.disconnect();
            walletAddress = null;
            console.log('Disconnected from wallet');
        }
    } catch (error) {
        console.error('Error disconnecting wallet:', error);
    }
}

// Check if a lottery is active by querying the blockchain

async function checkLotteryStatus(lotteryAccountAddress) {
    try {
        if (!solanaConnection || !lotteryAccountAddress) {
            return null;
        }
        
        // Checks if the lottery account exists on the blockchain. TODO: decode the data when contract is live
        const accountInfo = await solanaConnection.getAccountInfo(
            new solanaWeb3.PublicKey(lotteryAccountAddress)
        );
        
        if (!accountInfo) {
            console.log('Lottery account not found');
            return null;
        }
        
        // Placeholder until the contract is deployed
        return {
            isActive: true,
            prizeAmount: 15, 
            players: ['address_player_1', 'address_player_2', 'address_player_3'], 
        };
    } catch (error) {
        console.error('Error checking lottery status:', error);
        return null;
    }
}

// Get lottery status
async function getLotteryStatus(lotteryAccountAddress) {
    try {
        if (!solanaConnection || !lotteryAccountAddress) {
            throw new Error('No active lottery found');
        }
        const accountInfo = await solanaConnection.getAccountInfo(
            new PublicKey(lotteryAccountAddress)
        );
        if (!accountInfo) {
            throw new Error('Lottery account not found');
        }
        // Decode
        const lotteryData = await program.account.lottery.fetch(
            new PublicKey(lotteryAccountAddress)
        );
        return {
            isActive: lotteryData.isActive,
            prizeAmount: lotteryData.prizeAmount.toNumber() / 1e9,
            players: lotteryData.players.map(player => player.toString()),
        };
    } catch (error) {
        console.error('Error fetching lottery status:', error);
        displayError('Failed to fetch lottery status: ' + error.message);
        return null;
    }
}

// Start a new lottery
async function startLottery() {
    try {
        if (!solanaConnection || !walletAddress) {
            throw new Error('Please connect your wallet first');
        }
        
        // Check if user is admin
        if (!isAdmin(walletAddress)) {
            throw new Error('Only admin can start a lottery');
        }
        
        // TODO: call the smart contract to create a lottery account and start a lottery
        
        console.log('Starting new lottery');
        
        // Placeholder: create a simulated lottery account
        lotteryAccountPubkey = 'lottery_account_' + Date.now();
        return lotteryAccountPubkey;
    } catch (error) {
        console.error('Error starting lottery:', error);
        displayError('Failed to start lottery: ' + error.message);
        return null;
    }
}

// User participates in the lottery
async function participateInLottery(amount) {
    try {
        if (!solanaConnection || !walletAddress) {
            throw new Error('Please connect your wallet first');
        }

        if (!lotteryAccountPubkey) {
            throw new Error('No active lottery found');
        }

        if (!amount || isNaN(amount) || amount <= 0) {
            throw new Error('Please enter a valid amount in SOL');
        }

        // Convert amount to lamports (1 SOL = 1e9 lamports)
        const lamports = amount * 1e9;
        
        // Create the tx
        const tx = new Transaction().add(
            await lotteryProgramId.methods
                .participateInLottery(new BigInt(lamports))
                .accounts({
                    lottery: new PublicKey(lotteryAccountPubkey),
                    lotteryAccount: new PublicKey(lotteryAccountPubkey),
                    player: new PublicKey(walletAddress),
                    systemProgram: SystemProgram.programId,
                })
                .instruction()
        );
        
        const signature = await sendAndConfirmTransaction(
            solanaConnection,
            tx,
            [wallet.payer],
        );
        console.log(`Participating in lottery with ${amount} SOL`);

        return signature;
    } catch (error) {
        console.error('Error participating in lottery:', error);
        displayError('Failed to participate in lottery: ' + error.message);
        return null;
    }
}

// Draw a winner from the lottery
async function drawWinner() {
    try {
        if (!solanaConnection || !walletAddress) {
            throw new Error('Please connect your wallet first');
        }
        
        // Check if user is admin
        if (!isAdmin(walletAddress)) {
            throw new Error('Only admin can draw a winner');
        }
        
        if (!lotteryAccountPubkey) {
            throw new Error('No active lottery found');
        }
        
        // TODO: call the smart contract to draw a winner and return the winner's address
        
        console.log('Drawing winner');
        
        // Simulate a successful response
        const winner = 'simulated_winner_' + Date.now().toString().slice(-4);
        return {
            signature: 'simulated_transaction_' + Date.now(),
            winner: winner
        };
    } catch (error) {
        console.error('Error drawing winner:', error);
        displayError('Failed to draw winner: ' + error.message);
        return null;
    }
}


// Function to display errors on the front
function displayError(message) {
    const resultsElement = document.getElementById('results');
    if (resultsElement) {
        resultsElement.className = 'alert alert-danger';
        resultsElement.textContent = message;
    }
}

// Check if user is admin
function isAdmin(address) {
    return ADMIN_WALLET_ADDRESSES.includes(address);
}

// Initialize Solana connection when the page loads
document.addEventListener('DOMContentLoaded', initializeSolanaConnection);
