//! This module contains all the components needed to read and write data from files (specifically CSV)
//!
//! The [`reader`] module contains a reader of transactions from CSV and [`writer`] modules contains an account report writer into CSV.
//! It would be possible to add new file formats by implementing the traits [`TransactionsReader`] and [`AccountsReportWriter`] respectively.
//!
//! The [`account`] and [`transaction`] modules contain structs needed to serialize/deserialize data.
//! They are intentionally duplicated from the domain model to decouple the IO details from the domain logic and allow their evolution independently.
//!

mod account;
mod reader;
mod transaction;
mod writer;

pub use reader::{CsvTransactionsReader, TransactionsReader};
pub use writer::{AccountsReportWriter, CsvAccountsReportWriter};
