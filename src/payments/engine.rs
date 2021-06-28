use std::collections::HashMap;

use async_trait::async_trait;
use rust_decimal::Decimal;
use thiserror::Error;

use super::{
  account::{Account, AccountReport, TransactionState},
  transaction::{ClientId, Transaction, TransactionId},
};

/// Default decimal precision as number of decimals after the point
const PRECISION: u32 = 4;

pub type Result<T> = core::result::Result<T, PaymentsEngineError>;

/// Possible errors that can happen while processing transactions.
/// We are dealing with sensible information, so it is important to be as detailed as possible.
/// It could be observed through metrics or logs, or better used as events for a fraud system.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum PaymentsEngineError {
  #[error("Account is locked: {0}")]
  AccountLocked(ClientId),

  #[error("Invalid negative amount")]
  NegativeAmount,

  #[error("Not enough available funds")]
  NotEnoughAvailableFunds,

  #[error("Duplicated transaction: {0}")]
  DuplicatedTransaction(TransactionId),

  #[error("Client not found: {0}")]
  ClientNotFound(ClientId),

  #[error("Transaction not found: {0}")]
  TransactionNotFound(TransactionId),

  #[error("Transaction {1} for client {0} already disputed")]
  TransactionAlreadyDisputed(ClientId, TransactionId),

  #[error("Transaction {1} for client {0} is not disputed")]
  TransactionNotDisputed(ClientId, TransactionId),
}

/// Interface implemented by payments processors
#[async_trait]
pub trait PaymentsEngine {
  /// Operation called to process a transaction. It will return whether or not succeeded and detailed information about the error.
  /// The operation is `async` to allow interaction of the engine with external systems involving IO (database, file system, ...)
  async fn process(&mut self, transaction: Transaction) -> Result<()>;
  /// It will return an [`Iterator`] of [`AccountReport`] useful to generate account reports.
  fn accounts_report(&self) -> AccountsReportIter;
}

/// Implementation of the [`PaymentsEngine`] that uses memory to store accounts information and transactions.
#[derive(Debug)]
pub struct InMemoryPaymentsEngine {
  accounts: HashMap<ClientId, Account>,
}

impl InMemoryPaymentsEngine {
  pub fn new() -> Self {
    Self {
      accounts: HashMap::default(),
    }
  }

  fn deposit(
    &mut self,
    client_id: ClientId,
    transaction_id: TransactionId,
    amount: Decimal,
  ) -> Result<()> {
    if amount < Decimal::ZERO {
      Err(PaymentsEngineError::NegativeAmount)
    } else {
      let account = self.get_or_create_account(client_id);
      if account.locked {
        Err(PaymentsEngineError::AccountLocked(client_id))
      } else if account.transaction_exists(&transaction_id) {
        Err(PaymentsEngineError::DuplicatedTransaction(transaction_id))
      } else {
        account.funds.available += amount;
        account
          .transactions
          .insert(transaction_id, TransactionState::from_amount(amount));
        Ok(())
      }
    }
  }

  fn withdrawal(
    &mut self,
    client_id: ClientId,
    transaction_id: TransactionId,
    amount: Decimal,
  ) -> Result<()> {
    if amount < Decimal::ZERO {
      Err(PaymentsEngineError::NegativeAmount)
    } else {
      let account = self
        .accounts
        .get_mut(&client_id)
        .ok_or(PaymentsEngineError::ClientNotFound(client_id))?;

      if account.locked {
        Err(PaymentsEngineError::AccountLocked(client_id))
      } else if account.transaction_exists(&transaction_id) {
        Err(PaymentsEngineError::DuplicatedTransaction(transaction_id))
      } else if account.funds.available < amount {
        Err(PaymentsEngineError::NotEnoughAvailableFunds)
      } else {
        account.funds.available -= amount;
        account
          .transactions
          .insert(transaction_id, TransactionState::from_amount(-amount));
        Ok(())
      }
    }
  }

  fn dispute(&mut self, client_id: ClientId, transaction_id: TransactionId) -> Result<()> {
    let account = self
      .accounts
      .get_mut(&client_id)
      .ok_or(PaymentsEngineError::ClientNotFound(client_id))?;

    if account.locked {
      Err(PaymentsEngineError::AccountLocked(client_id))
    } else {
      let transaction = account
        .transactions
        .get_mut(&transaction_id)
        .ok_or(PaymentsEngineError::TransactionNotFound(transaction_id))?;

      if transaction.in_dispute {
        Err(PaymentsEngineError::TransactionAlreadyDisputed(
          client_id,
          transaction_id,
        ))
      } else {
        transaction.in_dispute = true;
        account.funds.available -= transaction.amount;
        account.funds.held += transaction.amount;
        Ok(())
      }
    }
  }

  fn resolve(&mut self, client_id: ClientId, transaction_id: TransactionId) -> Result<()> {
    let account = self
      .accounts
      .get_mut(&client_id)
      .ok_or(PaymentsEngineError::ClientNotFound(client_id))?;

    let transaction = account
      .transactions
      .get_mut(&transaction_id)
      .ok_or(PaymentsEngineError::TransactionNotFound(transaction_id))?;

    if !transaction.in_dispute {
      Err(PaymentsEngineError::TransactionNotDisputed(
        client_id,
        transaction_id,
      ))
    } else {
      transaction.in_dispute = false;
      account.funds.available += transaction.amount;
      account.funds.held -= transaction.amount;
      Ok(())
    }
  }

  fn chargeback(&mut self, client_id: ClientId, transaction_id: TransactionId) -> Result<()> {
    let account = self
      .accounts
      .get_mut(&client_id)
      .ok_or(PaymentsEngineError::ClientNotFound(client_id))?;

    let amount = {
      let transaction = account
        .transactions
        .get(&transaction_id)
        .ok_or(PaymentsEngineError::TransactionNotFound(transaction_id))?;

      if !transaction.in_dispute {
        Err(PaymentsEngineError::TransactionNotDisputed(
          client_id,
          transaction_id,
        ))
      } else {
        Ok(transaction.amount)
      }
    }?;

    account.locked = true;
    account.funds.held -= amount;
    account.transactions.remove(&transaction_id);

    Ok(())
  }

  fn get_or_create_account(&mut self, client_id: ClientId) -> &mut Account {
    self
      .accounts
      .entry(client_id)
      .or_insert_with(Account::default)
  }

  fn accounts_report_iter(&self) -> impl Iterator<Item = AccountReport> + '_ {
    self.accounts.iter().map(|(client_id, account)| {
      let total = account.funds.available + account.funds.held;
      AccountReport::new(
        *client_id,
        account.funds.available.round_dp(PRECISION),
        account.funds.held.round_dp(PRECISION),
        total.round_dp(PRECISION),
        account.locked,
      )
    })
  }
}

#[async_trait]
impl PaymentsEngine for InMemoryPaymentsEngine {
  async fn process(&mut self, transaction: Transaction) -> Result<()> {
    match transaction {
      Transaction::Deposit {
        client_id,
        transaction_id,
        amount,
      } => self.deposit(client_id, transaction_id, amount),
      Transaction::Withdrawal {
        client_id,
        transaction_id,
        amount,
      } => self.withdrawal(client_id, transaction_id, amount),
      Transaction::Dispute {
        client_id,
        transaction_id,
      } => self.dispute(client_id, transaction_id),
      Transaction::Resolve {
        client_id,
        transaction_id,
      } => self.resolve(client_id, transaction_id),
      Transaction::Chargeback {
        client_id,
        transaction_id,
      } => self.chargeback(client_id, transaction_id),
    }
  }

  fn accounts_report(&self) -> AccountsReportIter {
    AccountsReportIter::new(self.accounts_report_iter())
  }
}

pub struct AccountsReportIter<'a>(Box<dyn Iterator<Item = AccountReport> + 'a>);

impl<'a> AccountsReportIter<'a> {
  pub(crate) fn new<T>(iter: T) -> Self
  where
    T: Iterator<Item = AccountReport> + 'a,
  {
    Self(Box::new(iter))
  }
}

impl<'a> Iterator for AccountsReportIter<'a> {
  type Item = AccountReport;

  fn next(&mut self) -> Option<Self::Item> {
    self.0.next()
  }
}

#[cfg(test)]
mod tests {

  use std::collections::HashSet;

  use rust_decimal_macros::dec;

  use super::*;
  use crate::payments::account::Funds;

  #[tokio::test]
  async fn process_deposit_negative_amount() {
    let mut engine = InMemoryPaymentsEngine::new();
    let transaction = Transaction::Deposit {
      client_id: 1,
      transaction_id: 101,
      amount: dec!(-10),
    };

    let result = engine.process(transaction).await;

    assert_eq!(result, Err(PaymentsEngineError::NegativeAmount));
  }

  #[tokio::test]
  async fn process_deposit_account_locked() {
    let mut engine = InMemoryPaymentsEngine::new();
    engine.accounts.insert(
      1,
      Account {
        locked: true,
        ..Account::default()
      },
    );
    let transaction = Transaction::Deposit {
      client_id: 1,
      transaction_id: 101,
      amount: dec!(20),
    };

    let result = engine.process(transaction).await;

    assert_eq!(result, Err(PaymentsEngineError::AccountLocked(1)));
  }

  #[tokio::test]
  async fn process_deposit_transaction_exists() {
    let mut engine = InMemoryPaymentsEngine::new();
    engine.accounts.insert(
      1,
      Account {
        locked: false,
        funds: Funds::available(dec!(10)),
        transactions: vec![(101, TransactionState::from_amount(dec!(10)))]
          .into_iter()
          .collect(),
      },
    );
    let transaction = Transaction::Deposit {
      client_id: 1,
      transaction_id: 101,
      amount: dec!(20),
    };

    let result = engine.process(transaction).await;

    assert_eq!(result, Err(PaymentsEngineError::DuplicatedTransaction(101)));
  }

  #[tokio::test]
  async fn process_deposit_successfully() {
    let mut engine = InMemoryPaymentsEngine::new();
    engine.accounts.insert(1, Account::default());
    let transaction = Transaction::Deposit {
      client_id: 1,
      transaction_id: 101,
      amount: dec!(10),
    };

    let result = engine.process(transaction).await;

    assert_eq!(result, Ok(()));
    assert_eq!(engine.accounts.len(), 1);
    assert_eq!(
      engine.accounts.get(&1).unwrap(),
      &Account {
        locked: false,
        funds: Funds::available(dec!(10)),
        transactions: vec![(101, TransactionState::from_amount(dec!(10)))]
          .into_iter()
          .collect(),
      }
    );
  }

  #[tokio::test]
  async fn process_withdrawal_negative_amount() {
    let mut engine = InMemoryPaymentsEngine::new();
    let transaction = Transaction::Withdrawal {
      client_id: 1,
      transaction_id: 101,
      amount: dec!(-10),
    };

    let result = engine.process(transaction).await;

    assert_eq!(result, Err(PaymentsEngineError::NegativeAmount));
  }

  #[tokio::test]
  async fn process_withdrawal_account_locked() {
    let mut engine = InMemoryPaymentsEngine::new();
    engine.accounts.insert(
      1,
      Account {
        locked: true,
        ..Account::default()
      },
    );
    let transaction = Transaction::Withdrawal {
      client_id: 1,
      transaction_id: 101,
      amount: dec!(20),
    };

    let result = engine.process(transaction).await;

    assert_eq!(result, Err(PaymentsEngineError::AccountLocked(1)));
  }

  #[tokio::test]
  async fn process_withdrawal_transaction_exists() {
    let mut engine = InMemoryPaymentsEngine::new();
    engine.accounts.insert(
      1,
      Account {
        locked: false,
        funds: Funds::available(dec!(10)),
        transactions: vec![(101, TransactionState::from_amount(dec!(10)))]
          .into_iter()
          .collect(),
      },
    );
    let transaction = Transaction::Withdrawal {
      client_id: 1,
      transaction_id: 101,
      amount: dec!(5),
    };

    let result = engine.process(transaction).await;

    assert_eq!(result, Err(PaymentsEngineError::DuplicatedTransaction(101)));
  }

  #[tokio::test]
  async fn process_withdrawal_not_enough_available_funds() {
    let mut engine = InMemoryPaymentsEngine::new();
    engine.accounts.insert(
      1,
      Account {
        locked: false,
        funds: Funds::available(dec!(10)),
        transactions: HashMap::default(),
      },
    );
    let transaction1 = Transaction::Withdrawal {
      client_id: 1,
      transaction_id: 101,
      amount: dec!(10),
    };
    let transaction2 = Transaction::Withdrawal {
      client_id: 1,
      transaction_id: 102,
      amount: dec!(0.2),
    };

    let result = engine.process(transaction1).await;
    assert!(result.is_ok());

    let result = engine.process(transaction2).await;

    assert_eq!(result, Err(PaymentsEngineError::NotEnoughAvailableFunds));
  }

  #[tokio::test]
  async fn process_withdrawal_non_existing_client() {
    let mut engine = InMemoryPaymentsEngine::new();
    let transaction1 = Transaction::Withdrawal {
      client_id: 1,
      transaction_id: 101,
      amount: dec!(10),
    };

    let result = engine.process(transaction1).await;

    assert_eq!(result, Err(PaymentsEngineError::ClientNotFound(1)));
  }

  #[tokio::test]
  async fn process_withdrawal_successfully() {
    let mut engine = InMemoryPaymentsEngine::new();
    engine.accounts.insert(
      1,
      Account {
        locked: false,
        funds: Funds::available(dec!(100)),
        transactions: HashMap::default(),
      },
    );
    let transaction = Transaction::Withdrawal {
      client_id: 1,
      transaction_id: 101,
      amount: dec!(10),
    };

    let result = engine.process(transaction).await;

    assert!(result.is_ok());
    assert_eq!(engine.accounts.len(), 1);
    assert_eq!(
      engine.accounts.get(&1).unwrap(),
      &Account {
        locked: false,
        funds: Funds::available(dec!(90)),
        transactions: vec![(101, TransactionState::from_amount(dec!(-10)))]
          .into_iter()
          .collect(),
      }
    );
  }

  #[tokio::test]
  async fn process_dispute_non_existing_client() {
    let mut engine = InMemoryPaymentsEngine::new();
    let transaction = Transaction::Dispute {
      client_id: 1,
      transaction_id: 101,
    };

    let result = engine.process(transaction).await;

    assert_eq!(result, Err(PaymentsEngineError::ClientNotFound(1)));
  }

  #[tokio::test]
  async fn process_dispute_account_locked() {
    let mut engine = InMemoryPaymentsEngine::new();
    engine.accounts.insert(
      1,
      Account {
        locked: true,
        ..Account::default()
      },
    );
    let transaction = Transaction::Dispute {
      client_id: 1,
      transaction_id: 101,
    };

    let result = engine.process(transaction).await;

    assert_eq!(result, Err(PaymentsEngineError::AccountLocked(1)));
  }

  #[tokio::test]
  async fn process_dispute_non_existing_transaction() {
    let mut engine = InMemoryPaymentsEngine::new();
    engine.accounts.insert(
      1,
      Account {
        locked: false,
        funds: Funds::available(dec!(100)),
        transactions: HashMap::default(),
      },
    );
    let transaction = Transaction::Dispute {
      client_id: 1,
      transaction_id: 101,
    };

    let result = engine.process(transaction).await;

    assert_eq!(result, Err(PaymentsEngineError::TransactionNotFound(101)));
  }

  #[tokio::test]
  async fn process_dispute_already_disputed() {
    let mut engine = InMemoryPaymentsEngine::new();
    engine.accounts.insert(
      1,
      Account {
        locked: false,
        funds: Funds::available(dec!(100)),
        transactions: vec![(101, TransactionState::from_dispute(dec!(10)))]
          .into_iter()
          .collect(),
      },
    );
    let transaction = Transaction::Dispute {
      client_id: 1,
      transaction_id: 101,
    };

    let result = engine.process(transaction).await;

    assert_eq!(
      result,
      Err(PaymentsEngineError::TransactionAlreadyDisputed(1, 101))
    );
  }

  #[tokio::test]
  async fn process_dispute_successfully() {
    let mut engine = InMemoryPaymentsEngine::new();
    engine.accounts.insert(
      1,
      Account {
        locked: false,
        funds: Funds::available(dec!(110)),
        transactions: vec![(101, TransactionState::from_amount(dec!(10)))]
          .into_iter()
          .collect(),
      },
    );
    let transaction = Transaction::Dispute {
      client_id: 1,
      transaction_id: 101,
    };

    let result = engine.process(transaction).await;

    assert!(result.is_ok());
    assert_eq!(
      engine.accounts.get(&1).unwrap(),
      &Account {
        locked: false,
        funds: Funds::new(dec!(100), dec!(10)),
        transactions: vec![(101, TransactionState::from_dispute(dec!(10)))]
          .into_iter()
          .collect(),
      }
    );
  }

  #[tokio::test]
  async fn process_resolve_non_existing_client() {
    let mut engine = InMemoryPaymentsEngine::new();
    let transaction = Transaction::Resolve {
      client_id: 1,
      transaction_id: 101,
    };

    let result = engine.process(transaction).await;

    assert_eq!(result, Err(PaymentsEngineError::ClientNotFound(1)));
  }

  #[tokio::test]
  async fn process_resolve_non_existing_transaction() {
    let mut engine = InMemoryPaymentsEngine::new();
    engine.accounts.insert(
      1,
      Account {
        locked: false,
        funds: Funds::available(dec!(100)),
        transactions: HashMap::default(),
      },
    );
    let transaction = Transaction::Resolve {
      client_id: 1,
      transaction_id: 101,
    };

    let result = engine.process(transaction).await;

    assert_eq!(result, Err(PaymentsEngineError::TransactionNotFound(101)));
  }

  #[tokio::test]
  async fn process_resolve_not_disputed() {
    let mut engine = InMemoryPaymentsEngine::new();
    engine.accounts.insert(
      1,
      Account {
        locked: false,
        funds: Funds::available(dec!(100)),
        transactions: vec![(101, TransactionState::from_amount(dec!(10)))]
          .into_iter()
          .collect(),
      },
    );
    let transaction = Transaction::Resolve {
      client_id: 1,
      transaction_id: 101,
    };

    let result = engine.process(transaction).await;

    assert_eq!(
      result,
      Err(PaymentsEngineError::TransactionNotDisputed(1, 101))
    );
  }

  #[tokio::test]
  async fn process_resolve_successfully() {
    let mut engine = InMemoryPaymentsEngine::new();
    engine.accounts.insert(
      1,
      Account {
        locked: false,
        funds: Funds::new(dec!(100), dec!(10)),
        transactions: vec![(101, TransactionState::from_dispute(dec!(10)))]
          .into_iter()
          .collect(),
      },
    );
    let transaction = Transaction::Resolve {
      client_id: 1,
      transaction_id: 101,
    };

    let result = engine.process(transaction).await;

    assert!(result.is_ok());
    assert_eq!(
      engine.accounts.get(&1).unwrap(),
      &Account {
        locked: false,
        funds: Funds::available(dec!(110)),
        transactions: vec![(101, TransactionState::from_amount(dec!(10)))]
          .into_iter()
          .collect(),
      }
    );
  }

  #[tokio::test]
  async fn process_chargeback_non_existing_client() {
    let mut engine = InMemoryPaymentsEngine::new();
    let transaction = Transaction::Chargeback {
      client_id: 1,
      transaction_id: 101,
    };

    let result = engine.process(transaction).await;

    assert_eq!(result, Err(PaymentsEngineError::ClientNotFound(1)));
  }

  #[tokio::test]
  async fn process_chargeback_non_existing_transaction() {
    let mut engine = InMemoryPaymentsEngine::new();
    engine.accounts.insert(
      1,
      Account {
        locked: false,
        funds: Funds::available(dec!(100)),
        transactions: HashMap::default(),
      },
    );
    let transaction = Transaction::Chargeback {
      client_id: 1,
      transaction_id: 101,
    };

    let result = engine.process(transaction).await;

    assert_eq!(result, Err(PaymentsEngineError::TransactionNotFound(101)));
  }

  #[tokio::test]
  async fn process_chargeback_not_disputed() {
    let mut engine = InMemoryPaymentsEngine::new();
    engine.accounts.insert(
      1,
      Account {
        locked: false,
        funds: Funds::available(dec!(100)),
        transactions: vec![(101, TransactionState::from_amount(dec!(10)))]
          .into_iter()
          .collect(),
      },
    );
    let transaction = Transaction::Chargeback {
      client_id: 1,
      transaction_id: 101,
    };

    let result = engine.process(transaction).await;

    assert_eq!(
      result,
      Err(PaymentsEngineError::TransactionNotDisputed(1, 101))
    );
  }

  #[tokio::test]
  async fn process_chargeback_successfully() {
    let mut engine = InMemoryPaymentsEngine::new();
    engine.accounts.insert(
      1,
      Account {
        locked: false,
        funds: Funds::new(dec!(100), dec!(10)),
        transactions: vec![(101, TransactionState::from_dispute(dec!(10)))]
          .into_iter()
          .collect(),
      },
    );
    let transaction = Transaction::Chargeback {
      client_id: 1,
      transaction_id: 101,
    };

    let result = engine.process(transaction).await;

    assert!(result.is_ok());
    assert_eq!(
      engine.accounts.get(&1).unwrap(),
      &Account {
        locked: true,
        funds: Funds::available(dec!(100)),
        transactions: HashMap::default(),
      }
    );
  }

  #[test]
  fn accounts_report_empty() {
    let engine = InMemoryPaymentsEngine::new();

    let report: Vec<AccountReport> = engine.accounts_report().collect();

    assert_eq!(report, vec![]);
  }

  #[test]
  fn accounts_report_success() {
    let mut engine = InMemoryPaymentsEngine::new();
    engine.accounts.insert(
      1,
      Account {
        locked: false,
        funds: Funds::available(dec!(101.00015)),
        transactions: vec![(101, TransactionState::from_dispute(dec!(10)))]
          .into_iter()
          .collect(),
      },
    );
    engine.accounts.insert(
      2,
      Account {
        locked: false,
        funds: Funds::new(dec!(200.00005), dec!(-10)),
        ..Account::default()
      },
    );
    engine.accounts.insert(
      3,
      Account {
        locked: true,
        funds: Funds::available(dec!(300)),
        ..Account::default()
      },
    );

    let report: HashSet<AccountReport> = engine.accounts_report().collect();

    assert_eq!(
      report,
      vec![
        AccountReport::new(1, dec!(101.0002), dec!(0), dec!(101.0002), false),
        AccountReport::new(2, dec!(200.0000), dec!(-10), dec!(190.0000), false),
        AccountReport::new(3, dec!(300), dec!(0), dec!(300), true),
      ]
      .into_iter()
      .collect()
    );
  }
}
