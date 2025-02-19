// File: src/transaction/transaction.rs
// Project: Bifrost
// Creation date: Saturday 08 February 2025
// Author: Vincent Berthier <vincent.berthier@posteo.org>
// -----
// Last modified: Thursday 13 February 2025 @ 10:04:57
// Modified by: Vincent Berthier
// -----
// Copyright (c) 2025 <Vincent Berthier>
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the 'Software'), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED 'AS IS', WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use borsh::{BorshDeserialize, BorshSerialize};
use tracing::{debug, instrument, trace, warn};

use crate::crypto::{Keypair, Pubkey, Signature};

use super::{instruction::Instruction, message::Message, Error, Result};

/// A transaction to execute (or executed) on the Bifrost blockchain.
#[non_exhaustive]
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct Transaction {
    /// List of signatures for the message.
    signatures: Vec<Signature>,
    /// The message (compiled instructions).
    message: Message,
}

impl Transaction {
    /// Create a new transaction.
    ///
    /// # Parameters
    /// * `slot` - the slot at which (or after which) the transaction was created,
    #[must_use]
    pub const fn new(slot: u64) -> Self {
        Self {
            signatures: Vec::new(),
            message: Message::new(slot),
        }
    }

    /// Add instructions to the transaction.
    ///
    /// Note that it will clear any signatures if any.
    ///
    /// # Parameters
    /// * `instructions` - list of instructions to add to the transaction,
    ///
    /// # Errors
    /// Errors only happen if the same public key points to two different types
    /// of accounts (such as one is a wallet, the other a program).
    ///
    /// # Example
    /// ```rust
    /// # use bifrost::{
    ///     Error,
    ///     crypto::{Pubkey, Keypair},
    ///     account::{AccountMeta, Writable},
    ///     transaction::{Instruction, Transaction}
    /// };
    /// # const PROGRAM: Pubkey = Pubkey::from_bytes(&[2; 32]);
    /// let keypair = Keypair::generate();
    /// let mut trx = Transaction::new(0);
    /// let instruction = Instruction::new(PROGRAM, vec![AccountMeta::signing(keypair.pubkey(), Writable::Yes)?], &Vec::<u8>::new());
    /// trx.add(&[instruction])?;
    /// # Ok::<(), Error>(())
    /// ```
    #[instrument(skip_all)]
    pub fn add(&mut self, instructions: &[Instruction]) -> Result<()> {
        debug!(
            n = instructions.len(),
            "adding instructions to the transaction"
        );
        trace!("resetting signatures");
        self.signatures.clear();
        for instr in instructions {
            trace!("adding transaction");
            self.message.add_instruction(instr)?;
        }

        Ok(())
    }

    /// Sign a transaction.
    ///
    /// The payer's signature will always be used as the one
    /// used to designate the transaction in the future.
    ///
    /// # Parameters
    /// * `key` - the `keypair` of the signer,
    ///
    /// # Example
    /// ```rust
    /// # use bifrost::{
    ///     Error,
    ///     crypto::{Pubkey, Keypair},
    ///     account::{AccountMeta, Writable},
    ///     transaction::{Instruction, Transaction}
    /// };
    /// # const PROGRAM: Pubkey = Pubkey::from_bytes(&[2; 32]);
    /// let keypair = Keypair::generate();
    /// # let mut trx = Transaction::new(0);
    /// # let instruction = Instruction::new(PROGRAM, vec![AccountMeta::signing(keypair.pubkey(), Writable::Yes)?], &Vec::<u8>::new());
    /// # trx.add(&[instruction])?;
    /// trx.sign(&keypair)?;
    /// # Ok::<(), Error>(())
    /// ```
    #[expect(
        clippy::unwrap_used,
        clippy::unwrap_in_result,
        reason = "if we can sign, there’s a payer"
    )]
    #[instrument(skip_all, fields(?key))]
    pub fn sign(&mut self, key: &Keypair) -> Result<()> {
        debug!("signing transaction");
        let signature = self.get_signature(key)?;

        if key.pubkey() == self.message.get_payer().unwrap() {
            self.signatures.insert(0, signature);
        } else {
            self.signatures.push(signature);
        }

        Ok(())
    }

    #[instrument(skip_all, fields(?key))]
    fn get_signature(&self, key: &Keypair) -> Result<Signature> {
        debug!("get overall transaction signature");
        if !self.get_signers().contains(&key.pubkey()) {
            warn!("'{}' is not a signer for the transaction", key.pubkey());
            return Err(Error::UnexpectedSigner { key: key.pubkey() });
        }
        Ok(key.sign(self.message.to_vec()))
    }

    /// Checks that both the message and the signatures are valid.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.message.is_valid() && self.check_signed().is_ok()
    }

    /// Get the overall signature of the transaction (if it exists).
    ///
    /// If there are multiple signers, this will always be the one
    /// associated with the payer (*i.e.* the first referenced signing account).
    ///
    /// # Returns
    /// The transaction's signature if it exists
    #[expect(clippy::missing_const_for_fn, reason = "false positive")]
    #[must_use]
    pub fn signature(&self) -> Option<&Signature> {
        self.signatures.first()
    }

    #[instrument(skip_all)]
    fn check_signed(&self) -> Result<()> {
        debug!("checking transaction signatures");
        let signers = self.get_signers();

        if signers.is_empty() {
            warn!("there are no signers!");
            return Err(Error::NoSignersOnTransaction);
        }

        if signers.len() != self.signatures.len() {
            warn!("wrong number of signatures on the transaction");
            return Err(Error::WrongNumberOfSignatures {
                expected: signers.len(),
                actual: self.signatures.len(),
            });
        }
        self.validate_signers(&signers)
    }

    fn get_signers(&self) -> Vec<Pubkey> {
        self.message
            .accounts()
            .iter()
            .filter(|acc| acc.is_signing())
            .map(|meta| *meta.key())
            .collect::<Vec<_>>()
    }

    /// Get the message for the transaction.
    #[must_use]
    pub const fn message(&self) -> &Message {
        &self.message
    }

    #[instrument(skip_all)]
    fn validate_signers(&self, signers: &[Pubkey]) -> Result<()> {
        debug!("check that there’s a 1 to 1 match between signatures and signers");
        if !signers.iter().all(|signer| {
            self.signatures
                .iter()
                .any(|signature| signature.verify(signer, self.message.to_vec()).is_ok())
        }) {
            warn!("got an unexpected signature");
            return Err(Error::SignaturesMismatch);
        }
        trace!("transaction is signed");

        Ok(())
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {

    use std::assert_matches::assert_matches;

    use ed25519_dalek::PUBLIC_KEY_LENGTH;
    use test_log::test;

    use crate::account::{AccountMeta, Writable};

    use super::*;
    type Error = Box<dyn core::error::Error>;
    type TestResult = core::result::Result<(), Error>;

    pub const PROGRAM: Pubkey = Pubkey::from_bytes(&[2; PUBLIC_KEY_LENGTH]);

    fn get_instruction<A>(accounts: A) -> Instruction
    where
        A: Into<Vec<AccountMeta>>,
    {
        Instruction::new(PROGRAM, accounts.into(), &Vec::<u8>::new())
    }

    #[test]
    fn create_transaction() -> TestResult {
        // Given
        let keypair = Keypair::generate();
        let mut trx = Transaction::new(0);
        let instruction = get_instruction(vec![
            AccountMeta::signing(keypair.pubkey(), Writable::Yes)?,
            AccountMeta::wallet(keypair.pubkey(), Writable::No)?,
        ]);

        // When
        trx.add(&[instruction])?;
        trx.sign(&keypair)?;

        // Then
        assert!(trx.is_valid());

        Ok(())
    }

    #[test]
    fn adding_instruction_to_signed_trx() -> TestResult {
        // Given
        let keypair = Keypair::generate();
        let mut trx = Transaction::new(0);
        let instruction =
            get_instruction(vec![AccountMeta::signing(keypair.pubkey(), Writable::Yes)?]);
        trx.add(&[instruction.clone()])?;

        // When
        trx.sign(&keypair)?;
        trx.add(&[instruction])?;

        // Then
        assert!(!trx.is_valid());

        Ok(())
    }

    #[test]
    fn reject_unexpected_signer() -> TestResult {
        // Given
        let keypair = Keypair::generate();
        let mut trx = Transaction::new(0);
        let instruction =
            get_instruction(vec![AccountMeta::signing(keypair.pubkey(), Writable::Yes)?]);
        trx.add(&[instruction])?;

        let signer = Keypair::generate();

        // When
        let res = trx.sign(&signer);

        // Then
        assert_matches!(res, Err(super::super::Error::UnexpectedSigner { .. }));

        Ok(())
    }

    #[test]
    fn reject_invalid_signature() -> TestResult {
        // Given
        let keypair = Keypair::generate();
        let mut trx = Transaction::new(0);
        let instruction =
            get_instruction(vec![AccountMeta::signing(keypair.pubkey(), Writable::Yes)?]);
        trx.add(&[instruction])?;
        trx.sign(&keypair)?;

        // When
        let mut data = borsh::to_vec(&trx)?;
        data.iter_mut().skip(10).take(5).for_each(|v| *v = 0);
        let corrupted: Transaction = borsh::from_slice(&data)?;

        // Then
        assert!(!corrupted.is_valid());
        Ok(())
    }

    #[test]
    fn no_duplicate_account() -> TestResult {
        // Given
        let keypair = Keypair::generate();
        let mut trx = Transaction::new(0);
        let instruction1 =
            get_instruction(vec![AccountMeta::signing(keypair.pubkey(), Writable::Yes)?]);
        trx.add(&[instruction1])?;

        // When
        let instruction2 =
            get_instruction(vec![AccountMeta::wallet(keypair.pubkey(), Writable::No)?]);
        trx.add(&[instruction2])?;

        // Then
        assert_eq!(trx.message.accounts().len(), 2); // One for the signer, one for the program

        Ok(())
    }

    #[test]
    fn merge_writable_accounts() -> TestResult {
        // Given
        let keypair = Keypair::generate();
        let key2 = Keypair::generate().pubkey();
        let mut trx = Transaction::new(0);
        let instruction1 = get_instruction(vec![
            AccountMeta::signing(keypair.pubkey(), Writable::Yes)?,
            AccountMeta::wallet(key2, Writable::No)?,
        ]);
        trx.add(&[instruction1])?;
        let instruction2 = get_instruction(vec![AccountMeta::wallet(key2, Writable::Yes)?]);

        // When
        trx.add(&[instruction2])?;

        // Then
        let account = trx
            .message
            .accounts()
            .iter()
            .find(|acc| *acc.key() == key2)
            .ok_or("could not find the account")?;
        assert!(account.is_writable());
        Ok(())
    }

    #[test]
    fn merge_signing_accounts() -> TestResult {
        // Given
        let keypair = Keypair::generate();
        let key2 = Keypair::generate().pubkey();
        let mut trx = Transaction::new(0);
        let instruction1 = get_instruction(vec![
            AccountMeta::signing(keypair.pubkey(), Writable::Yes)?,
            AccountMeta::wallet(key2, Writable::No)?,
        ]);
        trx.add(&[instruction1])?;
        let instruction2 = get_instruction(vec![AccountMeta::signing(key2, Writable::Yes)?]);

        // When
        trx.add(&[instruction2])?;

        // Then
        let account = trx
            .message
            .accounts()
            .iter()
            .find(|acc| *acc.key() == key2)
            .ok_or("could not find the account")?;
        assert!(account.is_signing());
        Ok(())
    }

    #[test]
    fn same_trx_different_time_different_signature() -> TestResult {
        // Given
        let keypair = Keypair::generate();
        let instruction = get_instruction(vec![
            AccountMeta::signing(keypair.pubkey(), Writable::Yes)?,
            AccountMeta::wallet(keypair.pubkey(), Writable::No)?,
        ]);

        let mut trx1 = Transaction::new(0);
        trx1.add(&[instruction.clone()])?;

        let mut trx2 = Transaction::new(1);
        trx2.add(&[instruction])?;

        // When
        trx1.sign(&keypair)?;
        trx2.sign(&keypair)?;

        // Then
        assert_ne!(trx1.signatures, trx2.signatures);
        Ok(())
    }

    #[test]
    fn trx_signature_is_first_signers() -> TestResult {
        // Given
        let payer = Keypair::generate();
        let signer = Keypair::generate();
        let mut trx = Transaction::new(0);
        let instruction = get_instruction(vec![
            AccountMeta::signing(payer.pubkey(), Writable::Yes)?,
            AccountMeta::signing(signer.pubkey(), Writable::No)?,
        ]);
        trx.add(&[instruction])?;
        trx.sign(&signer)?;
        trx.sign(&payer)?;
        let expected = payer.sign(trx.message.to_vec());

        // When
        let signature = trx.signature();

        // Then
        assert_matches!(signature, Some(sig) if *sig == expected);
        Ok(())
    }
}
