# Solana Lottery Game Frontend

Frontend for the game

## Structure

- `index.html` - The main HTML file that defines the structure of the UI
- `styles.css` - Custom CSS styles to complement Bootstrap
- `js/index.js` - Main application logic for UI interactions
- `js/solana.js` - Solana blockchain interaction functions

## How to Run

We can run this in several ways. I use a simple http server using python but it is also possible to use node. 


```bash
# For Python 3
python -m http.server
```

Implemented:

1. **Wallet Connection**: Connect to Phantom wallet
2. **Start Lottery**: Create a new lottery with a specified prize amount
3. **Participate in Lottery**: Join an active lottery
4. **Draw Winner**: Select a winner for the lottery

## TODO:

## Required Smart Contract Endpoints

1. Start Lottery
2. Participate in Lottery
3. Draw Winner
4. Get Lottery Status
5. 
