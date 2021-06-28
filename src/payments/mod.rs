//! This module contains the domain logic to process transactions
//!
//! The [`InMemoryPaymentsEngine`] is a dummy implementation of a [`PaymentsEngine`] that uses memory to store accounts information and transactions.
//

mod account;
mod engine;
mod transaction;

pub(crate) use account::AccountReport;

#[cfg(test)]
pub(crate) use engine::Result as EngineResult;

pub use engine::{AccountsReportIter, InMemoryPaymentsEngine, PaymentsEngine, PaymentsEngineError};
pub use transaction::{ClientId, Transaction, TransactionId};
