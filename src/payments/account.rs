use std::collections::HashMap;

use rust_decimal::Decimal;

use super::{transaction::TransactionId, ClientId};

/// This represents the state of a client account while processing transactions
#[derive(Debug, PartialEq)]
pub struct Account {
  pub locked: bool,
  pub funds: Funds,
  pub transactions: HashMap<TransactionId, TransactionState>,
}

impl Account {
  pub fn transaction_exists(&self, transaction_id: &TransactionId) -> bool {
    self.transactions.contains_key(transaction_id)
  }
}

impl Default for Account {
  fn default() -> Self {
    Self {
      locked: false,
      funds: Funds::zero(),
      transactions: HashMap::default(),
    }
  }
}

/// This represents the state of a recorded transaction.
#[derive(Debug, PartialEq)]
pub struct TransactionState {
  /// The `amount` will be positive for deposits and negative for withdrawals.
  pub amount: Decimal,
  /// The `in_dispute` will tell whether the transaction is being disputed or not.
  pub in_dispute: bool,
}

impl TransactionState {
  #[cfg(test)]
  pub fn from_dispute(amount: Decimal) -> Self {
    Self {
      amount,
      in_dispute: true,
    }
  }

  pub fn from_amount(amount: Decimal) -> Self {
    Self {
      amount,
      in_dispute: false,
    }
  }
}

/// Representation of the different states in which funds can be, either available or in held.
#[derive(Debug, PartialEq)]
pub struct Funds {
  pub available: Decimal,
  pub held: Decimal,
}

impl Funds {
  #[cfg(test)]
  pub fn new(available: Decimal, held: Decimal) -> Self {
    Self { available, held }
  }

  pub fn zero() -> Self {
    Self {
      available: Decimal::ZERO,
      held: Decimal::ZERO,
    }
  }

  #[cfg(test)]
  pub fn available(available: Decimal) -> Self {
    Self {
      available,
      held: Decimal::ZERO,
    }
  }
}

/// Account report structure used to export information about the state of the client accounts.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccountReport {
  pub client_id: ClientId,
  pub available: Decimal,
  pub held: Decimal,
  pub total: Decimal,
  pub locked: bool,
}

impl AccountReport {
  pub fn new(
    client_id: ClientId,
    available: Decimal,
    held: Decimal,
    total: Decimal,
    locked: bool,
  ) -> Self {
    Self {
      client_id,
      available,
      held,
      total,
      locked,
    }
  }
}

#[cfg(test)]
mod tests {

  use rust_decimal_macros::dec;

  use super::*;

  #[test]
  fn account_transaction_exists() {
    let account = Account {
      transactions: vec![(101, TransactionState::from_amount(dec!(10)))]
        .into_iter()
        .collect(),
      ..Account::default()
    };

    assert!(account.transaction_exists(&101));
    assert!(!account.transaction_exists(&202));
  }

  #[test]
  fn transaction_state_constructors() {
    assert_eq!(
      TransactionState::from_dispute(dec!(10)),
      TransactionState {
        amount: dec!(10),
        in_dispute: true
      }
    );

    assert_eq!(
      TransactionState::from_amount(dec!(10)),
      TransactionState {
        amount: dec!(10),
        in_dispute: false
      }
    );
  }

  #[test]
  fn funds_constructors() {
    assert_eq!(
      Funds::new(dec!(1), dec!(2)),
      Funds {
        available: dec!(1),
        held: dec!(2)
      }
    );

    assert_eq!(
      Funds::zero(),
      Funds {
        available: dec!(0),
        held: dec!(0)
      }
    );

    assert_eq!(
      Funds::available(dec!(1)),
      Funds {
        available: dec!(1),
        held: dec!(0)
      }
    );
  }

  #[test]
  fn account_report_constructor() {
    assert_eq!(
      AccountReport::new(1, dec!(100), dec!(10), dec!(110), true),
      AccountReport {
        client_id: 1,
        available: dec!(100),
        held: dec!(10),
        total: dec!(110),
        locked: true,
      }
    )
  }
}
