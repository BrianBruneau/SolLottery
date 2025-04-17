// UI interactions and connects HTML to solana interface

const connectWalletBtn = document.getElementById('connect-wallet');
const walletStatusEl = document.getElementById('wallet-connection-status');
const walletAddressEl = document.getElementById('wallet-address');
const lotteryStatusEl = document.getElementById('lottery-status');
const lotteryDetailsEl = document.getElementById('lottery-details');
const prizeAmountEl = document.getElementById('prize-amount');
const playerCountEl = document.getElementById('player-count');
const adminSectionEl = document.getElementById('admin-section');
const playerSectionEl = document.getElementById('player-section');
const startLotteryBtn = document.getElementById('start-lottery');
const prizeInputEl = document.getElementById('prize-input');
const participationAmountEl = document.getElementById('participation-amount');
const participateBtn = document.getElementById('participate');
const drawWinnerBtn = document.getElementById('draw-winner');
const resultsEl = document.getElementById('results');

// app state
let isWalletConnected = false;
let isLotteryActive = false;

// Init app
function initApp() {
    console.log('Initializing Lottery Game App');
    
    // event listeners
    connectWalletBtn.addEventListener('click', handleWalletConnection);
    startLotteryBtn.addEventListener('click', handleStartLottery);
    participateBtn.addEventListener('click', handleParticipate);
    drawWinnerBtn.addEventListener('click', handleDrawWinner);
    
    // hide admin and player sections until wallet is connected
    adminSectionEl.classList.add('d-none');
    playerSectionEl.classList.add('d-none');
}

// Wallet connections / disconnections
async function handleWalletConnection() {
    if (!isWalletConnected) {
        // Connect wallet
        showLoading(connectWalletBtn, 'Connecting...');
        
        const address = await connectWallet();
        
        if (address) {
            isWalletConnected = true;
            updateWalletUI(address);
            checkForActiveLottery();
            hideLoading(connectWalletBtn, '🔓 Disconnect Wallet');
        } else {
            hideLoading(connectWalletBtn, '🔗 Connect Wallet');
        }
    } else {
        // Disconnect wallet
        showLoading(connectWalletBtn, 'Disconnecting...');
        await disconnectWallet();
        isWalletConnected = false;
        updateWalletUI(null);
        
        // Hide sections when disconnected
        adminSectionEl.classList.add('d-none');
        playerSectionEl.classList.add('d-none');
        
        hideLoading(connectWalletBtn, '🔗 Connect Wallet');
    }
}

//Update the UI based on wallet connection status

function updateWalletUI(address) {
    if (address) {
        // Wallet connected
        walletStatusEl.innerHTML = '<span class="badge bg-success">Wallet Connected</span>';
        walletAddressEl.textContent = `Address: ${address}`;
        connectWalletBtn.textContent = '🔓 Disconnect Wallet';
        connectWalletBtn.classList.remove('btn-primary');
        connectWalletBtn.classList.add('btn-outline-danger');
        
        // Show player section for all connected wallets
        playerSectionEl.classList.remove('d-none');
        
        // Show admin section only if the connected wallet is an admin
        if (isAdmin(address)) {
            adminSectionEl.classList.remove('d-none');
            console.log('Admin wallet detected, showing admin panel');
        } else {
            adminSectionEl.classList.add('d-none');
        }
        
        // Log to confirm button text was updated
        console.log('Wallet connected, button text updated to:', connectWalletBtn.textContent);
    } else {
        // Wallet disconnected
        walletStatusEl.innerHTML = '<span class="badge bg-warning text-dark">Wallet not connected</span>';
        walletAddressEl.textContent = '';
        connectWalletBtn.textContent = '🔗 Connect Wallet';
        connectWalletBtn.classList.add('btn-primary');
        connectWalletBtn.classList.remove('btn-outline-danger');
        
        // Hide sections when disconnected
        adminSectionEl.classList.add('d-none');
        playerSectionEl.classList.add('d-none');
        
        // Reset lottery status
        lotteryStatusEl.className = 'alert alert-info';
        lotteryStatusEl.textContent = 'No active lottery found.';
        lotteryDetailsEl.classList.add('d-none');
        isLotteryActive = false;
    }
}


// Check if active lottery: TODO query blockchain
async function checkForActiveLottery() {
    // Simulate until sc deployed
    updateLotteryStatusUI({
        isActive: true,
        prizeAmount: 10,
        players: ['player1', 'player2']
    });
    
}

function updateLotteryStatusUI(lotteryData) {
    if (lotteryData && lotteryData.isActive) {
        // Active lottery found
        lotteryStatusEl.className = 'alert alert-success';
        lotteryStatusEl.innerHTML = `Lottery is active! <br><small class="text-muted">ID: ${lotteryAccountPubkey}</small>`;
        
        // Show lottery details
        lotteryDetailsEl.classList.remove('d-none');
        prizeAmountEl.textContent = lotteryData.prizeAmount || 0;
        playerCountEl.textContent = lotteryData.players ? lotteryData.players.length : 0;
        
        // Update buttons
        drawWinnerBtn.disabled = false;
        participateBtn.disabled = false;
        
        isLotteryActive = true;
    } else {
        // No active lottery
        lotteryStatusEl.className = 'alert alert-info';
        lotteryStatusEl.textContent = 'No active lottery found.';
        lotteryDetailsEl.classList.add('d-none');
        
        // Update buttons
        drawWinnerBtn.disabled = true;
        participateBtn.disabled = true;
        
        isLotteryActive = false;
    }
}


// Owner can start lottery
async function handleStartLottery() {
    if (!isWalletConnected) {
        displayError('Please connect your wallet first');
        return;
    }
    
    // Check if user is admin
    if (!isAdmin(walletAddress)) {
        displayError('Only admin can start a lottery');
        return;
    }
    
    showLoading(startLotteryBtn, 'Starting...');

    // Start a new lottery
    const lotteryAddress = await startLottery();
    
    if (lotteryAddress) {
        // Success - update UI
        displaySuccess(`Lottery started successfully! Address: ${lotteryAddress}`);
        
        // Update lottery status - prize will be accumulated from participants
        updateLotteryStatusUI({
            isActive: true,
            prizeAmount: 0, // Initial prize is 0, will accumulate from bets
            players: []
        });
    }
    
    hideLoading(startLotteryBtn, '🚀 Start New Lottery');
}

async function handleParticipate() {
    if (!isWalletConnected) {
        displayError('Please connect your wallet first');
        return;
    }
    
    if (!isLotteryActive) {
        displayError('No active lottery found');
        return;
    }
    
    // Get amount from input field
    const amount = parseFloat(participationAmountEl.value);
    
    if (isNaN(amount) || amount <= 0) {
        displayError('Please enter a valid amount in SOL');
        return;
    }
    
    showLoading(participateBtn, 'Joining...');
    
    const txSignature = await participateInLottery(amount);
    
    if (txSignature) {
        // Success - update UI
        displaySuccess(`Successfully joined the lottery with ${amount} SOL! Transaction: ${txSignature}`);
        
        // Update player count TODO
        const currentCount = parseInt(playerCountEl.textContent || '0');
        playerCountEl.textContent = currentCount + 1;
    }
    
    hideLoading(participateBtn, 'Participate');
}


async function handleDrawWinner() {
    if (!isWalletConnected) {
        displayError('Please connect your wallet first');
        return;
    }
    
    // Check if user is admin
    if (!isAdmin(walletAddress)) {
        displayError('Only admin can draw a winner');
        return;
    }
    
    if (!isLotteryActive) {
        displayError('No active lottery found');
        return;
    }
    
    showLoading(drawWinnerBtn, 'Drawing...');
    
    // Call the Solana function to draw a winner
    const result = await drawWinner();
    
    if (result) {
        // Success - update UI
        displaySuccess(`Winner drawn! Winner address: ${result.winner}`);
        
        // Update lottery status
        updateLotteryStatusUI(null);
    }
    
    hideLoading(drawWinnerBtn, '🏆 Draw Winner');
}

function displaySuccess(message) {
    resultsEl.className = 'alert alert-success';
    resultsEl.textContent = message;
}

function displayError(message) {
    resultsEl.className = 'alert alert-danger';
    resultsEl.textContent = message;
}

function showLoading(button, loadingText) {
    button.disabled = true;
    button.dataset.originalText = button.textContent;
    button.textContent = loadingText;
    button.classList.add('loading');
}

function hideLoading(button, text) {
    button.disabled = false;
    button.textContent = text || button.dataset.originalText;
    button.classList.remove('loading');
}

// Initialize the app when the DOM is fully loaded
document.addEventListener('DOMContentLoaded', initApp);
