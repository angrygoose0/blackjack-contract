#![allow(clippy::result_large_err)]

use anchor_lang::{
    prelude::*,
    solana_program::{
        pubkey::Pubkey,
        keccak,
    },
    system_program,
};

use ephemeral_vrf_sdk::anchor::vrf;
use ephemeral_vrf_sdk::instructions::{create_request_randomness_ix, RequestRandomnessParams};
use ephemeral_vrf_sdk::types::SerializableAccountMeta;
use ephemeral_rollups_sdk::anchor::{commit, delegate, ephemeral};
use ephemeral_rollups_sdk::cpi::DelegateConfig;
use ephemeral_rollups_sdk::ephem::{commit_accounts, commit_and_undelegate_accounts};

use anchor_spl::{
    associated_token::{AssociatedToken},
    token_interface::{Mint, TokenAccount, TokenInterface, transfer_checked, TransferChecked, SyncNative, sync_native},
};

/*
use light_sdk::{
    account::LightAccount,
    address::v1::derive_address,
    cpi::{accounts::CompressionCpiAccounts, verify::verify_compressed_account_infos},
    error::LightSdkError,
    instruction::{account_meta::CompressedAccountMeta, instruction_data::LightInstructionData},
    NewAddressParamsPacked,
};

use light_account_checks::discriminator::Discriminator;
use light_sdk_macros::{LightDiscriminator, LightHasher};

*/


declare_id!("Bp3EKTavjkMo6uLHgvnbF6VeL8xWs4uA6tNvJxndRrr");


#[ephemeral]
#[program]
pub mod blackjack {
    use super::*;

    //mainnet: pub const TOKEN_MINT: Pubkey = pubkey!("5gVSqhk41VA8U6U4Pvux6MSxFWqgptm3w58X9UTGpump"); //mainnet
    pub const TOKEN_MINT: Pubkey = pubkey!("D2BYx2UoshNpAfgBEXEEyfUKxLSxkLMAb6zeZhZYgoos"); //devnet
    pub const TOKEN_DECIMALS: u8 = 9;

    pub const FEE_PERCENTAGE: u8 = 100; // divide by 100 so 1%

    pub const SOLANA_MINT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");

    pub const TREASURY_SEED: &[u8] = b"2e6d1f85819918f17a2847743e4fb707";

    pub fn init_treasuries(
        ctx: Context<InitializeTreasuries>,
    ) -> Result<()> {

        require!(
            ctx.accounts.solana_mint.key() == SOLANA_MINT,
            CustomError::Unauthorized,
        );

        require!(
            ctx.accounts.token_mint.key() == TOKEN_MINT,
            CustomError::Unauthorized,
        );

        Ok(())
    }

    /*
    pub fn create_light_deck<'info>(
        ctx: Context<'_, '_, '_, 'info, CreateLightDeck<'info>>,
        light_ix_data: LightInstructionData,
        output_merkle_tree_index: u8,
    ) -> Result<()> {
        let program_id = crate::ID.into();

        let light_cpi_accounts = CompressionCpiAccounts::new(
            ctx.accounts.signer.as_ref(),
            ctx.remaining_accounts,
            crate::ID,
        ).map_err(ProgramError::from)?;

        let address_merkle_context = light_ix_data
            .new_addresses
            .ok_or(LightSdkError::ExpectedAddressMerkleContext)
            .map_err(ProgramError::from)?[0];

        let (address, address_seed) = derive_address(
            &[b"DECK", ctx.accounts.blackjack.key().as_ref()],
            &light_cpi_accounts.tree_accounts()
                [address_merkle_context.address_merkle_tree_pubkey_index as usize]
                .key(),
            &crate::ID,
        );

        let new_address_params = NewAddressParamsPacked {
            seed: address_seed,
            address_queue_account_index: address_merkle_context.address_queue_pubkey_index,
            address_merkle_tree_root_index: address_merkle_context.root_index,
            address_merkle_tree_account_index: address_merkle_context
                .address_merkle_tree_pubkey_index,
        };

        let mut deck = LightAccount::<'_, Deck2>::new_init(
            &program_id,
            Some(address),
            output_merkle_tree_index,
        );

        //deck.cards = [0; 52];
        deck.blackjack = ctx.accounts.blackjack.key();
        deck.drawn = 0;

        verify_compressed_account_infos(
            &light_cpi_accounts,
            light_ix_data.proof,
            &[deck.to_account_info().unwrap()],
            Some(vec![new_address_params]),
            None,
            false,
            None,
        )
        .map_err(ProgramError::from)?;


        Ok(())
    }   
    */


    pub fn join_blackjack(
        ctx: Context<JoinBlackJack>,
    ) -> Result<()> {

        let blackjack = &mut ctx.accounts.blackjack;

        blackjack.player = ctx.accounts.signer.key();
        blackjack.active_hands = 0;

        blackjack.total_owed = 0;

        blackjack.dealer_card_1 = 0;
        blackjack.dealer_card_2 = 0;
        blackjack.dealer_card_3 = 0;
        blackjack.dealer_card_4 = 0;
        blackjack.dealer_card_5 = 0;
        blackjack.dealer_card_6 = 0;


        Ok(())
    }


    //only call after joining before anteing.
    pub fn shuffle_deck(ctx: Context<DoShuffleDeckCtx>, client_seed: u8) -> Result<()> {
        let blackjack = &ctx.accounts.blackjack;

        require!(
            blackjack.active_hands == 0,
            CustomError::Unauthorized
        );

        msg!("Requesting randomness...");
        let ix = create_request_randomness_ix(RequestRandomnessParams {
            payer: ctx.accounts.signer.key(),
            oracle_queue: ctx.accounts.oracle_queue.key(),
            callback_program_id: ID,
            callback_discriminator: instruction::CallbackShuffleDeck::DISCRIMINATOR.to_vec(),
            caller_seed: [client_seed; 32],
            // Specify any account that is required by the callback
            accounts_metas: Some(vec![
                SerializableAccountMeta {
                    pubkey: ctx.accounts.deck.key(),
                    is_signer: false,
                    is_writable: true,
                }
            ]),
            ..Default::default()
        });
        ctx.accounts
            .invoke_signed_vrf(&ctx.accounts.signer.to_account_info(), &ix)?;
        Ok(())
    }
  
    pub fn callback_shuffle_deck(
        ctx: Context<CallbackShuffleDeckCtx>,
        randomness: [u8; 32],
    ) -> Result<()> {
        let deck = &mut ctx.accounts.deck;
        
        // Use the helper function to get a shuffled deck
        let shuffled = shuffled_deck_from_seed(randomness);
        deck.cards.copy_from_slice(&shuffled);
        deck.drawn = 0;

        Ok(())
    }

    //delegate blackjack
    pub fn delegate_blackjack(
        ctx: Context<DelegateBlackJack>,
    ) -> Result<()> {

        ctx.accounts.delegate_blackjack(
            &ctx.accounts.signer,
            &[b"BLACKJACK", ctx.accounts.signer.key().to_bytes().as_slice()],
            DelegateConfig::default(),
        )?;

        ctx.accounts.delegate_deck(
            &ctx.accounts.signer,
            &[b"DECK", ctx.accounts.blackjack.key().to_bytes().as_slice()],
            DelegateConfig::default(),
        )?;

        
        Ok(())
    }

    //delegate blackjack hand
    pub fn delegate_blackjack_hand(
        ctx: Context<DelegateBlackJackHand>,
        hand_id: u8,
    ) -> Result<()> {


        ctx.accounts.delegate_blackjack_hand(
            &ctx.accounts.signer,
            &[b"BLACKJACKHAND", ctx.accounts.blackjack.key().to_bytes().as_slice(), &[hand_id]],
            DelegateConfig::default(),
        )?;

        Ok(())
    }



    pub fn commit_undelegate_blackjack(
        ctx: Context<CommitBlackJack>,
    ) -> Result<()> {
        commit_and_undelegate_accounts(
            &ctx.accounts.signer,
            vec![&ctx.accounts.blackjack.to_account_info()],
            &ctx.accounts.magic_context,
            &ctx.accounts.magic_program,
        )?;

        commit_and_undelegate_accounts(
            &ctx.accounts.signer,
            vec![&ctx.accounts.deck.to_account_info()],
            &ctx.accounts.magic_context,
            &ctx.accounts.magic_program,
        )?;

        Ok(())
    }

    pub fn commit_undelegate_blackjack_hand(
        ctx: Context<CommitBlackJackHand>,
        _hand_id: u8,
    ) -> Result<()> {

        commit_and_undelegate_accounts(
            &ctx.accounts.signer,
            vec![&ctx.accounts.blackjack_hand.to_account_info()],
            &ctx.accounts.magic_context,
            &ctx.accounts.magic_program,
        )?;
    
        Ok(())
    }
    

    //delegate new hand and deck and blackjack
    pub fn ante_blackjack( // if blackjack, stand automatically, will pay 1.5x when finishing turn | if dealer's first card is ACE, give players an option for insurance, if dealer's first card is ace / face card, do a random roll to see if its blackjack or not. if it is, the hand is stood automatically, and the player can only do next_turn, which will take the money, or give back the original bet if player has blackjack, or give some back because of the insurance bet.
        ctx: Context<AnteBlackJack>,
        hand_id: u8,
        player_bet: u64,
    ) -> Result<()> {

        require!(
            hand_id == 1,
            CustomError::Unauthorized
        );

        transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    to: ctx.accounts.token_treasury.to_account_info(),
                    authority: ctx.accounts.signer.to_account_info(),
                    mint: ctx.accounts.token_mint.to_account_info(),
                },   
            ),
            player_bet,
            TOKEN_DECIMALS,
        );

        let blackjack = &mut ctx.accounts.blackjack;

        require!(
            blackjack.dealer_card_1 == 0,
            CustomError::Unauthorized
        );


        blackjack.active_hands += 1;
        require!(
            blackjack.active_hands == hand_id,
            CustomError::Unauthorized
        );

        let deck = &mut ctx.accounts.deck;

        /*
        let clock = Clock::get()?;
        let seed = keccak::hash(&clock.unix_timestamp.to_le_bytes()).0;
        let shuffled = shuffled_deck_from_seed(seed);
        deck.cards.copy_from_slice(&shuffled);
        deck.drawn = 0;
        */


        let card_1 = deck.cards[deck.drawn as usize];
        deck.drawn += 1;
        let card_2 = deck.cards[deck.drawn as usize];
        deck.drawn += 1;
        let card_3 = deck.cards[deck.drawn as usize];
        deck.drawn += 1;

        let val_1 = get_card_value(card_1, true);
        let val_2 = get_card_value(card_2, true);
        let val_3 = get_card_value(card_3, true);

        blackjack.dealer_card_1 = card_3;

        {
            let blackjack_hand = &mut ctx.accounts.blackjack_hand;

            blackjack_hand.blackjack = blackjack.key();
            blackjack_hand.id = hand_id;
            blackjack_hand.state = 0;
            blackjack_hand.current_bet = player_bet;
            blackjack_hand.insured = false;
            

            blackjack_hand.player_card_1 = card_1; // give
            blackjack_hand.player_card_2 = card_2; //give
            blackjack_hand.player_card_3 = 0;
            blackjack_hand.player_card_4 = 0;
            blackjack_hand.player_card_5 = 0;
            blackjack_hand.player_card_6 = 0;
            blackjack_hand.player_card_7 = 0;
            blackjack_hand.player_card_8 = 0;
            blackjack_hand.player_card_9 = 0;
            blackjack_hand.player_card_10 = 0;

            // Check for blackjack
            if (val_1 == 11 && val_2 == 10) || (val_1 == 10 && val_2 == 11) {
                blackjack_hand.state = 3;
            }
        }


        if val_3 == 11 {
            ctx.accounts.blackjack_hand.state = 1; //insurance state
        }

        Ok(())
    }

    //should be delegated here.
    pub fn insurance_blackjack(
        ctx: Context<InsuranceBlackJack>,
        _hand_id: u8,
        insurance: bool
    ) -> Result<()> {
        let deck = &ctx.accounts.deck;

        require!(deck.drawn < 52, CustomError::Unauthorized);

        {
            let blackjack_hand = &mut ctx.accounts.blackjack_hand;

            require!(
                blackjack_hand.state == 1,
                CustomError::Unauthorized
            );

            if insurance == true {
                transfer_checked(
                    CpiContext::new(
                        ctx.accounts.token_program.to_account_info(),
                        TransferChecked {
                            from: ctx.accounts.user_token_account.to_account_info(),
                            to: ctx.accounts.token_treasury.to_account_info(),
                            authority: ctx.accounts.signer.to_account_info(),
                            mint: ctx.accounts.token_mint.to_account_info(),
                        },   
                    ),
                    blackjack_hand.current_bet / 2,
                    TOKEN_DECIMALS,
                );

                blackjack_hand.insured = true;
            }

            blackjack_hand.state = 0;
        }

        /*
        commit_accounts(
            &ctx.accounts.signer,
            vec![&ctx.accounts.blackjack_hand.to_account_info()],
            &ctx.accounts.magic_context,
            &ctx.accounts.magic_program,
        )?;
        */

        Ok(())
    }
    
    //should be delegated here.
    pub fn hit_blackjack(
        ctx: Context<HitBlackJack>,
        _hand_id: u8,
    ) -> Result<()> {
        

        {
            let deck = &mut ctx.accounts.deck;

            require!(deck.drawn < 52, CustomError::Unauthorized);

            let blackjack_hand = &mut ctx.accounts.blackjack_hand;

            require!(
                blackjack_hand.state == 0,
                CustomError::Unauthorized
            );

            let new_card = deck.cards[deck.drawn as usize];
            deck.drawn += 1;

            if blackjack_hand.player_card_3 == 0 {
                blackjack_hand.player_card_3 = new_card;
            } else if blackjack_hand.player_card_4 == 0 {
                blackjack_hand.player_card_4 = new_card;
            } else if blackjack_hand.player_card_5 == 0 {
                blackjack_hand.player_card_5 = new_card;
            } else if blackjack_hand.player_card_6 == 0 {
                blackjack_hand.player_card_6 = new_card;
            } else if blackjack_hand.player_card_7 == 0 {
                blackjack_hand.player_card_7 = new_card;
            } else if blackjack_hand.player_card_8 == 0 {
                blackjack_hand.player_card_8 = new_card;
            } else if blackjack_hand.player_card_9 == 0 {
                blackjack_hand.player_card_9 = new_card;
            } else if blackjack_hand.player_card_10 == 0 {
                blackjack_hand.player_card_10 = new_card;
            } else {
                // All slots are filled – maybe return an error?
                return Err(error!(CustomError::Unauthorized));
            }

            let total: u8 = get_card_value(blackjack_hand.player_card_1, false)
                + get_card_value(blackjack_hand.player_card_2, false)
                + get_card_value(blackjack_hand.player_card_3, false)
                + get_card_value(blackjack_hand.player_card_4, false)
                + get_card_value(blackjack_hand.player_card_5, false)
                + get_card_value(blackjack_hand.player_card_6, false)
                + get_card_value(blackjack_hand.player_card_7, false)
                + get_card_value(blackjack_hand.player_card_8, false)
                + get_card_value(blackjack_hand.player_card_9, false)
                + get_card_value(blackjack_hand.player_card_10, false);

            if total > 21 {
                blackjack_hand.state = 2; //BUST

            }
        }

        /*
        commit_accounts(
            &ctx.accounts.signer,
            vec![&ctx.accounts.blackjack_hand.to_account_info()],
            &ctx.accounts.magic_context,
            &ctx.accounts.magic_program,
        )?;

        commit_accounts(
            &ctx.accounts.signer,
            vec![&ctx.accounts.deck.to_account_info()],
            &ctx.accounts.magic_context,
            &ctx.accounts.magic_program,
        )?;

        */

        


        Ok(())
    }

    //should be delegated here.
    pub fn stand_blackjack( // locks in their hand, gets dealers random cards, determines who won,
        ctx: Context<HitBlackJack>,
        _hand_id: u8,
    ) -> Result<()> {
        {
            let blackjack_hand = &mut ctx.accounts.blackjack_hand;

            require!(
                blackjack_hand.state == 0,
                CustomError::Unauthorized
            );

            blackjack_hand.state = 3;
        }

        /*
        commit_accounts(
            &ctx.accounts.signer,
            vec![&ctx.accounts.blackjack_hand.to_account_info()],
            &ctx.accounts.magic_context,
            &ctx.accounts.magic_program,
        )?;
        */
        
        Ok(())
    }

    // undelegate all accounts. hands, decks, blackjack.
    // and then delegate back.
    pub fn split_blackjack( // check if card_1 = card_2, if so, make a second instance of blackjackhand with an id using blackjack.active_hands, and then make it so we give them both new card_2, while keeping original card_1 as card_1., if the card being split is ace, players cannot hit more, and stands automatically.
        ctx: Context<SplitBlackJack>,
        _hand_id: u8,
        new_hand_id: u8,
    ) -> Result<()> {
    
        {
            let blackjack = &mut ctx.accounts.blackjack;
            let deck = &mut ctx.accounts.deck;
            let blackjack_hand = &mut ctx.accounts.blackjack_hand;
            let new_blackjack_hand = &mut ctx.accounts.new_blackjack_hand;
            
            require!(
                blackjack_hand.state == 0,
                CustomError::Unauthorized
            );

            require!(
                blackjack_hand.player_card_3 == 0,
                CustomError::Unauthorized
            );

            blackjack.active_hands += 1;

            require!(
                blackjack.active_hands == new_hand_id,
                CustomError::Unauthorized
            );

            transfer_checked(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.user_token_account.to_account_info(),
                        to: ctx.accounts.token_treasury.to_account_info(),
                        authority: ctx.accounts.signer.to_account_info(),
                        mint: ctx.accounts.token_mint.to_account_info(),
                    },   
                ),
                blackjack_hand.current_bet,
                TOKEN_DECIMALS,
            );

            let new_card_1 = deck.cards[deck.drawn as usize];
            deck.drawn += 1;

            let new_card_2 = deck.cards[deck.drawn as usize];
            deck.drawn += 1;
            
            blackjack_hand.player_card_2 = new_card_1; // new card

            new_blackjack_hand.blackjack = blackjack.key();
            new_blackjack_hand.current_bet = blackjack_hand.current_bet;
            new_blackjack_hand.insured = false;
            new_blackjack_hand.id = new_hand_id;
            new_blackjack_hand.player_card_1 = blackjack_hand.player_card_1;
            new_blackjack_hand.player_card_2 = new_card_2; //new card.
            new_blackjack_hand.player_card_3 = 0;
            new_blackjack_hand.player_card_4 = 0;
            new_blackjack_hand.player_card_5 = 0;
            new_blackjack_hand.player_card_6 = 0;
            new_blackjack_hand.player_card_7 = 0;
            new_blackjack_hand.player_card_8 = 0;
            new_blackjack_hand.player_card_9 = 0;
            new_blackjack_hand.player_card_10 = 0;

            if get_card_value(blackjack_hand.player_card_1, true) == 11 {
                blackjack_hand.state = 3;
                new_blackjack_hand.state = 3;
            } else {
                let total: u8 = get_card_value(blackjack_hand.player_card_1, false)
                + get_card_value(blackjack_hand.player_card_2, false);

                let new_total: u8 = get_card_value(new_blackjack_hand.player_card_1, false)
                + get_card_value(new_blackjack_hand.player_card_2, false);

                if total > 21 {
                    blackjack_hand.state = 2; //BUST
                } 
                if new_total > 21 {
                    new_blackjack_hand.state = 2;
                }
            }
        }
   
        Ok(())
    }
    
    //should be delegated here.
    pub fn double_blackjack(// put in your current_bet again, update black_hand.current_bet *= 2, stands this hand automatically
        ctx: Context<InsuranceBlackJack>,
        _hand_id: u8,
    ) -> Result<()> {
    
        {
            let blackjack_hand = &mut ctx.accounts.blackjack_hand;
            let deck = &mut ctx.accounts.deck;

            require!(
                blackjack_hand.state == 0,
                CustomError::Unauthorized
            );

            require!(
                blackjack_hand.player_card_3 == 0,
                CustomError::Unauthorized
            );

            
            transfer_checked(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.user_token_account.to_account_info(),
                        to: ctx.accounts.token_treasury.to_account_info(),
                        authority: ctx.accounts.signer.to_account_info(),
                        mint: ctx.accounts.token_mint.to_account_info(),
                    },   
                ),
                blackjack_hand.current_bet,
                TOKEN_DECIMALS,
            );

            blackjack_hand.current_bet *= 2;

            let new_card = deck.cards[deck.drawn as usize];
            deck.drawn += 1;

            blackjack_hand.player_card_3 = new_card;
            blackjack_hand.state = 3;

            let total: u8 = get_card_value(blackjack_hand.player_card_1, false)
                + get_card_value(blackjack_hand.player_card_2, false)
                + get_card_value(blackjack_hand.player_card_3, false);

            if total > 21 {
                blackjack_hand.state = 2; //BUST  
            }
        }

        /*
        commit_accounts(
            &ctx.accounts.signer,
            vec![&ctx.accounts.blackjack_hand.to_account_info()],
            &ctx.accounts.magic_context,
            &ctx.accounts.magic_program,
        )?;

        commit_accounts(
            &ctx.accounts.signer,
            vec![&ctx.accounts.deck.to_account_info()],
            &ctx.accounts.magic_context,
            &ctx.accounts.magic_program,
        )?;
        */
        

        Ok(())
    }

    //still delegated here.
    pub fn dealer_turn( // goes through each blackjack hand with a remaining account, make sure all hands have been stood or busted. sees which lost and which lost, pays out or not, deletes all the blackjack hand instances make blackjack.active_hands = 0
        ctx: Context<DealerTurn>,
    ) -> Result<()> {
        {
            let blackjack = &mut ctx.accounts.blackjack;
            let deck = &mut ctx.accounts.deck;

            require!(
                ctx.remaining_accounts.len() == blackjack.active_hands as usize,
                CustomError::Unauthorized
            );

            require!(
                blackjack.dealer_card_2 == 0,
                CustomError::Unauthorized
            );

            require!(
                blackjack.total_owed == 0,
                CustomError::Unauthorized
            );


            let new_card_1 = deck.cards[deck.drawn as usize];
            deck.drawn += 1;

            blackjack.dealer_card_2 = new_card_1;

            let mut dealer_cards = vec![
                blackjack.dealer_card_1,
                blackjack.dealer_card_2,
            ];

            let mut dealer_total: u8 = 0;

            while dealer_cards.len() < 6 {
                // Calculate total and count aces
                dealer_total = 0;

                let mut ace_used_as_eleven = false;
                for card in &dealer_cards {
                    let mut val = get_card_value(*card, false);
                    if val == 1 && !ace_used_as_eleven {
                        val = 11;
                        ace_used_as_eleven = true;
                    }
                    dealer_total += val;
                    
                }

                if dealer_total > 21 && ace_used_as_eleven {
                    dealer_total -= 10;
                }

                // Dealer stands on hard 17+, hits on soft 17
                if dealer_total > 17 || (dealer_total == 17 && !ace_used_as_eleven) {
                    break;
                }

                // Draw next card
                let next = deck.cards[deck.drawn as usize];
                deck.drawn += 1;
                dealer_cards.push(next);

                // Save to struct
                match dealer_cards.len() {
                    3 => blackjack.dealer_card_3 = next,
                    4 => blackjack.dealer_card_4 = next,
                    5 => blackjack.dealer_card_5 = next,
                    6 => blackjack.dealer_card_6 = next,
                    _ => {}
                }
            }

            if dealer_total > 21 {
                dealer_total = 1;
            }

            let mut total_owed: u64 = 0;

            for account in ctx.remaining_accounts.iter() {
                let data = account.try_borrow_mut_data().map_err(|_| CustomError::Unauthorized)?;
                
                let blackjack_hand_instance = BlackJackHand::try_deserialize(&mut &data[..])
                    .map_err(|_| CustomError::Unauthorized)?;
            
                require!(
                    blackjack_hand_instance.blackjack == blackjack.key(),
                    CustomError::Unauthorized
                );

                let mut payout: u64 = 0;
            
                if blackjack_hand_instance.state != 2 { // Not busted
                    let cards = [
                        blackjack_hand_instance.player_card_1,
                        blackjack_hand_instance.player_card_2,
                        blackjack_hand_instance.player_card_3,
                        blackjack_hand_instance.player_card_4,
                        blackjack_hand_instance.player_card_5,
                        blackjack_hand_instance.player_card_6,
                        blackjack_hand_instance.player_card_7,
                        blackjack_hand_instance.player_card_8,
                        blackjack_hand_instance.player_card_9,
                        blackjack_hand_instance.player_card_10,
                    ];
            
                    let mut ace_used_as_eleven = false;
                    let mut total: u8 = 0;

                    
            
                    for &card in cards.iter() {
                        let mut val = get_card_value(card, false);
                        if val == 1 && !ace_used_as_eleven {
                            val = 11;
                            ace_used_as_eleven = true;
                        }
                        total += val;
                    }

            
                    if total > 21 && ace_used_as_eleven {
                        total -= 10;
                    }
            
                    if total <= 21 {
                        if total == dealer_total {
                            payout += blackjack_hand_instance.current_bet;
                        } else if total > dealer_total {
                            if total == 21 {
                                payout += blackjack_hand_instance.current_bet * 5 / 2;
                            } else {
                                payout += blackjack_hand_instance.current_bet * 2;
                            }
                        }
                    }
                }

                if dealer_total == 21 && blackjack_hand_instance.insured {
                    payout += blackjack_hand_instance.current_bet * 3 / 2;
                }

                total_owed += payout;
            }
            

            blackjack.total_owed = total_owed;

        }

        /*
        commit_accounts(
            &ctx.accounts.signer,
            vec![&ctx.accounts.blackjack.to_account_info()],
            &ctx.accounts.magic_context,
            &ctx.accounts.magic_program,
        )?;

        commit_accounts(
            &ctx.accounts.signer,
            vec![&ctx.accounts.deck.to_account_info()],
            &ctx.accounts.magic_context,
            &ctx.accounts.magic_program,
        )?;
        */

        Ok(())
    }


    // commit and undelegate all accounts here. deck, hand, blackjack. do a remaining accounts loop.
    pub fn finish_game(
        ctx: Context<FinishGame>,
    ) -> Result<()> {
        let blackjack = &mut ctx.accounts.blackjack;

        require!(
            blackjack.dealer_card_2 != 0,
            CustomError::Unauthorized
        );

        require!(
            ctx.remaining_accounts.len() == blackjack.active_hands as usize,
            CustomError::Unauthorized
        );

        for account in ctx.remaining_accounts.iter() {
            let data = account.try_borrow_mut_data().map_err(|_| CustomError::Unauthorized)?;
            
            let blackjack_hand_instance = BlackJackHand::try_deserialize(&mut &data[..])
                .map_err(|_| CustomError::Unauthorized)?;
        
            require!(
                blackjack_hand_instance.blackjack == blackjack.key(),
                CustomError::Unauthorized
            );

            // Transfer lamports to the signer before closing
            **ctx.accounts.signer.to_account_info().lamports.borrow_mut() += account.lamports();
            **account.lamports.borrow_mut() = 0;
        
            // Mark the account for closing
            **account.try_borrow_mut_lamports()? = 0;
        }
        
        if blackjack.total_owed != 0 {
            let seeds = &["TOKEN".as_bytes(), &[ctx.bumps.token_treasury]];
            let signer = [&seeds[..]];

            transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.token_treasury.to_account_info(),
                        to: ctx.accounts.user_token_account.to_account_info(),
                        authority: ctx.accounts.token_treasury.to_account_info(),
                        mint: ctx.accounts.token_mint.to_account_info(),
                    },
                    &signer,
                ),
                blackjack.total_owed,
                TOKEN_DECIMALS,
            )?;
        }

        blackjack.active_hands = 0;
        blackjack.total_owed = 0;
        blackjack.dealer_card_1 = 0;
        blackjack.dealer_card_2 = 0;
        blackjack.dealer_card_3 = 0;
        blackjack.dealer_card_4 = 0;
        blackjack.dealer_card_5 = 0;
        blackjack.dealer_card_6 = 0;

        Ok(())
    }
    

    
}

fn shuffled_deck_from_seed(seed: [u8; 32]) -> [u8; 52] {
    let mut deck: [u8; 52] = core::array::from_fn(|i| (i + 1) as u8);
    for i in (1..deck.len()).rev() {
        let hash = keccak::hashv(&[&seed, &[i as u8]]);
        let j = (hash.0[0] as usize) % (i + 1);
        deck.swap(i, j);
    }
    deck
}

fn get_card_value(card_id: u8, ace_high: bool) -> u8 {
    if card_id == 0 {
        return 0;
    }

    let rank = (card_id - 1) % 13 + 1;
    match rank {
        1 => if ace_high { 11 } else { 1 },
        11 | 12 | 13 => 10,
        _ => rank,
    }
}

#[vrf]
#[derive(Accounts)]
pub struct DoShuffleDeckCtx<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"BLACKJACK", signer.key().as_ref()],
        bump
    )]
    pub blackjack: Account<'info, BlackJack>,

    #[account(
        init,
        payer = signer,
        space = 8 + std::mem::size_of::<Deck>(), // 8 for discriminator
        seeds = [b"DECK", blackjack.key().as_ref()],
        bump
    )]
    pub deck: Account<'info, Deck>,

    /// CHECK: The oracle queue
    #[account(mut, address = ephemeral_vrf_sdk::consts::DEFAULT_QUEUE)]
    pub oracle_queue: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct CallbackShuffleDeckCtx<'info> {
    /// This check ensure that the vrf_program_identity (which is a PDA) is a singer
    /// enforcing the callback is executed by the VRF program trough CPI
    #[account(address = ephemeral_vrf_sdk::consts::VRF_PROGRAM_IDENTITY)]
    pub vrf_program_identity: Signer<'info>,

    #[account(mut)]
    pub deck: Account<'info, Deck>,
}

#[derive(Accounts)]
pub struct InitializeTreasuries<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(mut)]
    pub solana_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init,
        seeds = [b"SOLANA"],
        bump,
        payer = signer,
        token::mint = solana_mint,
        token::authority = solana_treasury,
    )]
    pub solana_treasury: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub token_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init,
        seeds = [b"TOKEN", TREASURY_SEED],
        bump,
        payer = signer,
        token::mint = token_mint,
        token::authority = token_treasury,
    )]
    pub token_treasury: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct JoinBlackJack<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        init,
        payer = signer,
        space = 8 + BlackJack::INIT_SPACE,
        seeds = [b"BLACKJACK", signer.key().as_ref()],
        bump
    )]
    pub blackjack: Account<'info, BlackJack>,

    pub system_program: Program<'info, System>,
}

#[delegate]
#[derive(Accounts)]
pub struct DelegateBlackJack<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"BLACKJACK", signer.key().as_ref()],
        del,
        bump
    )]
    pub blackjack: Account<'info, BlackJack>,

    #[account(
        mut,
        del,
        seeds = [b"DECK", blackjack.key().as_ref()],
        bump
    )]
    pub deck: Account<'info, Deck>,

    pub system_program: Program<'info, System>,
}

#[delegate]
#[derive(Accounts)]
#[instruction(hand_id: u8)]
pub struct DelegateBlackJackHand<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"BLACKJACK", signer.key().as_ref()],
        bump
    )]
    pub blackjack: Account<'info, BlackJack>,

    #[account(
        mut,
        seeds = [b"BLACKJACKHAND", blackjack.key().as_ref(), &[hand_id]],
        del,
        bump
    )]
    pub blackjack_hand: Account<'info, BlackJackHand>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(
    hand_id: u8,
)]
pub struct AnteBlackJack<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"BLACKJACK", signer.key().as_ref()],
        bump
    )]
    pub blackjack: Account<'info, BlackJack>,

    #[account(
        mut,
        seeds = [b"DECK", blackjack.key().as_ref()],
        bump
    )]
    pub deck: Account<'info, Deck>,

    #[account(
        init,
        payer = signer,
        space = 8 + BlackJackHand::INIT_SPACE,
        seeds = [b"BLACKJACKHAND", blackjack.key().as_ref(), &[hand_id]],
        bump
    )]
    pub blackjack_hand: Account<'info, BlackJackHand>,

    #[account(
        mut,
        seeds = [b"TOKEN", TREASURY_SEED],
        bump,
    )]
    pub token_treasury: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub token_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init_if_needed,
        payer = signer,
        associated_token::mint = token_mint,
        associated_token::authority = signer,
    )]
    pub user_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
#[instruction(
    hand_id: u8,
)]
pub struct InsuranceBlackJack<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"BLACKJACK", signer.key().as_ref()],
        bump
    )]
    pub blackjack: Account<'info, BlackJack>,

    #[account(
        mut,
        seeds = [b"DECK", blackjack.key().as_ref()],
        bump
    )]
    pub deck: Account<'info, Deck>,

    #[account(
        mut,
        seeds = [b"BLACKJACKHAND", blackjack.key().as_ref(), &[hand_id]],
        bump
    )]
    pub blackjack_hand: Account<'info, BlackJackHand>,

    #[account(
        mut,
        seeds = [b"TOKEN", TREASURY_SEED],
        bump,
    )]
    pub token_treasury: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub token_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init_if_needed,
        payer = signer,
        associated_token::mint = token_mint,
        associated_token::authority = signer,
    )]
    pub user_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}


#[derive(Accounts)]
#[instruction(
    hand_id: u8,
    new_hand_id: u8,
)]
pub struct SplitBlackJack<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"BLACKJACK", signer.key().as_ref()],
        bump
    )]
    pub blackjack: Account<'info, BlackJack>,
    

    #[account(
        mut,
        seeds = [b"DECK", blackjack.key().as_ref()],
        bump
    )]
    pub deck: Account<'info, Deck>,


    #[account(
        mut,
        seeds = [b"BLACKJACKHAND", blackjack.key().as_ref(), &[hand_id]],
        bump
    )]
    pub blackjack_hand: Account<'info, BlackJackHand>,

    #[account(
        init,
        payer = signer,
        space = 8 + BlackJackHand::INIT_SPACE,
        seeds = [b"BLACKJACKHAND", blackjack.key().as_ref(), &[new_hand_id]],
        bump
    )]
    pub new_blackjack_hand: Account<'info, BlackJackHand>,


    #[account(
        mut,
        seeds = [b"TOKEN", TREASURY_SEED],
        bump,
    )]
    pub token_treasury: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub token_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init_if_needed,
        payer = signer,
        associated_token::mint = token_mint,
        associated_token::authority = signer,
    )]
    pub user_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,

}

#[derive(Accounts)]
#[instruction(
    hand_id: u8,
)]
pub struct HitBlackJack<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"BLACKJACK", signer.key().as_ref()],
        bump
    )]
    pub blackjack: Account<'info, BlackJack>,

    #[account(
        mut,
        seeds = [b"DECK", blackjack.key().as_ref()],
        bump
    )]
    pub deck: Account<'info, Deck>,

    #[account(
        mut,
        seeds = [b"BLACKJACKHAND", blackjack.key().as_ref(), &[hand_id]],
        bump
    )]
    pub blackjack_hand: Account<'info, BlackJackHand>,

    pub system_program: Program<'info, System>,

}

#[derive(Accounts)]
pub struct DealerTurn<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"BLACKJACK", signer.key().as_ref()],
        bump
    )]
    pub blackjack: Account<'info, BlackJack>,

    #[account(
        mut,
        seeds = [b"DECK", blackjack.key().as_ref()],
        bump
    )]
    pub deck: Account<'info, Deck>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FinishGame<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"BLACKJACK", signer.key().as_ref()],
        bump
    )]
    pub blackjack: Account<'info, BlackJack>,

    #[account(
        mut,
        close = signer,
        seeds = [b"DECK", blackjack.key().as_ref()],
        bump
    )]
    pub deck: Account<'info, Deck>,

    #[account(
        mut,
        seeds = [b"TOKEN", TREASURY_SEED],
        bump,
    )]
    pub token_treasury: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub token_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init_if_needed,
        payer = signer,
        associated_token::mint = token_mint,
        associated_token::authority = signer,
    )]
    pub user_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}


#[commit]
#[derive(Accounts)]
#[instruction(hand_id: u8)]
pub struct CommitBlackJackHand<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"BLACKJACK", signer.key().as_ref()],
        bump
    )]
    pub blackjack: Account<'info, BlackJack>,

    #[account(
        mut,
        seeds = [b"BLACKJACKHAND", blackjack.key().as_ref(), &[hand_id]],
        bump
    )]
    pub blackjack_hand: Account<'info, BlackJackHand>,


    pub system_program: Program<'info, System>,
}

#[commit]
#[derive(Accounts)]
pub struct CommitBlackJack<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"BLACKJACK", signer.key().as_ref()],
        bump
    )]
    pub blackjack: Account<'info, BlackJack>,

    #[account(
        mut,
        seeds = [b"DECK", blackjack.key().as_ref()],
        bump
    )]
    pub deck: Account<'info, Deck>,

    pub system_program: Program<'info, System>,
}


#[derive(Accounts)]
pub struct CreateLightDeck<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    
    #[account(
        mut,
        seeds = [b"BLACKJACK", signer.key().as_ref()],
        bump
    )]
    pub blackjack: Account<'info, BlackJack>,
    

    pub system_program: Program<'info, System>,
}


#[account]
#[derive(InitSpace)]
pub struct BlackJackHand {
    pub blackjack: Pubkey,
    pub id: u8,

    pub state: u8, // 0 playing | 1 insurance | 2 busted | 3 stood (only one that has potential to win) make sure all hands are either 2 or 3 when finishing

    pub current_bet: u64,
    pub insured: bool,

    pub player_card_1: u8,
    pub player_card_2: u8,
    pub player_card_3: u8,
    pub player_card_4: u8,
    pub player_card_5: u8,
    pub player_card_6: u8,
    pub player_card_7: u8,
    pub player_card_8: u8,
    pub player_card_9: u8,
    pub player_card_10: u8,
    
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct BlackJack {
    pub player: Pubkey,
    pub active_hands: u8,
    pub total_owed: u64,

    pub dealer_card_1: u8,
    pub dealer_card_2: u8,
    pub dealer_card_3: u8,
    pub dealer_card_4: u8,
    pub dealer_card_5: u8,
    pub dealer_card_6: u8,


    pub bump: u8,
}

#[error_code]
pub enum CustomError {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Too Small")]
    TooSmall,
    #[msg("Too Big")]
    TooBig
}

#[account]
#[derive(InitSpace)]
pub struct Deck {
    pub cards: [u8; 52],      // The shuffled cards (1..=52)
    pub drawn: u8,            // Number of cards already drawn (0 initially)
    pub bump: u8,             // For PDA
}

/*
#[derive(
    Clone, Debug, Default, AnchorDeserialize, AnchorSerialize, LightHasher, LightDiscriminator,
)]
pub struct Deck2 {
    #[hash]
    pub blackjack : Pubkey,
    //pub cards: [u8; 52],      // The shuffled cards (1..=52)
    pub drawn: u8,            // Number of cards already drawn (0 initially)
}
*/

