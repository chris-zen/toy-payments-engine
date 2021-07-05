use rust_decimal::Decimal;
use serde::Serialize;

use crate::payments::{self, ClientId};

const MAX_PRECISION: u32 = 4;

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
      available: with_max_precission(account_report.available),
      held: with_max_precission(account_report.held),
      total: with_max_precission(account_report.total),
      locked: account_report.locked,
    }
  }
}

fn with_max_precission(mut value: Decimal) -> Decimal {
  if value.scale() > MAX_PRECISION {
    value.rescale(MAX_PRECISION);
  }
  if value.is_zero() {
    value = Decimal::ZERO;
  }
  value
}

#[cfg(test)]
mod tests {

  use super::*;
  use rust_decimal_macros::dec;

  #[test]
  fn from_payments_account_report() {
    let payments_account_report =
      payments::AccountReport::new(1, dec!(100.12345), dec!(10.012345), dec!(110.5678), false);

    let account_report: AccountReport = payments_account_report.into();

    assert_eq!(
      account_report,
      AccountReport {
        client: 1,
        available: dec!(100.1235),
        held: dec!(10.0123),
        total: dec!(110.5678),
        locked: false
      }
    )
  }

  #[test]
  fn with_max_precission_rescales() {
    let cases = vec![
      (dec!(0.00), "0"),
      (dec!(0.00000), "0"),
      (dec!(0.00004), "0"),
      (dec!(0.00005), "0.0001"),
      (dec!(1.23456789), "1.2346"),
    ];

    for (input, expected) in cases {
      assert_eq!(format!("{}", with_max_precission(input)).as_str(), expected);
    }
  }
}
