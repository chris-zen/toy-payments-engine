use rust_decimal::Decimal;
use serde::Serialize;

use crate::payments::{self, ClientId};

/// A report on an account state used to serialize into a CSV file
#[derive(Debug, PartialEq, Serialize)]
pub struct AccountReport {
  client: ClientId,
  available: Decimal,
  held: Decimal,
  total: Decimal,
  locked: bool,
}

impl From<payments::AccountReport> for AccountReport {
  /// A conversion between the domain representation of an account report into a serializable structure
  fn from(account_report: payments::AccountReport) -> Self {
    AccountReport {
      client: account_report.client_id,
      available: account_report.available,
      held: account_report.held,
      total: account_report.total,
      locked: account_report.locked,
    }
  }
}

#[cfg(test)]
mod tests {

  use super::*;
  use rust_decimal_macros::dec;

  #[test]
  fn from_payments_account_report() {
    let payments_account_report =
      payments::AccountReport::new(1, dec!(100), dec!(10), dec!(110), false);

    let account_report: AccountReport = payments_account_report.into();

    assert_eq!(
      account_report,
      AccountReport {
        client: 1,
        available: dec!(100),
        held: dec!(10),
        total: dec!(110),
        locked: false
      }
    )
  }
}
