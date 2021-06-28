use anyhow::Result;
use async_trait::async_trait;
use tokio::io::AsyncWrite;
use tokio_stream::StreamExt;

use crate::payments::AccountReport;

/// Interface for an account report writer
#[async_trait(?Send)]
pub trait AccountsReportWriter {
  /// Write the accounts information provided by the [`Iterator`] and return whether the operation was successful or not.
  async fn write_accounts_report<'a, T>(&'a mut self, report: T) -> Result<()>
  where
    T: Iterator<Item = AccountReport> + 'a;
}

/// An implementation of [`AccountsReportWriter`] for the CSV format.
pub struct CsvAccountsReportWriter<W>(W);

impl<W> CsvAccountsReportWriter<W>
where
  W: AsyncWrite + Unpin + Send + Sync,
{
  pub fn new(writer: W) -> Self {
    Self(writer)
  }
}

#[async_trait(?Send)]
impl<W> AccountsReportWriter for CsvAccountsReportWriter<W>
where
  W: AsyncWrite + Unpin + Send + Sync,
{
  async fn write_accounts_report<'a, T>(&'a mut self, report: T) -> Result<()>
  where
    T: Iterator<Item = AccountReport> + 'a,
  {
    let mut report = Box::pin(tokio_stream::iter(
      report.map(super::account::AccountReport::from),
    ));

    let mut serializer = csv_async::AsyncSerializer::from_writer(&mut self.0);
    while let Some(account_report) = report.next().await {
      serializer.serialize(account_report).await?;
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {

  use rust_decimal_macros::dec;
  use std::io::Cursor;
  use std::iter;

  use super::*;

  #[tokio::test]
  async fn write_accounts_report_fails() {
    let buff: &mut [u8] = &mut [0u8, 0, 0, 0];
    let mut buffer = Cursor::new(buff);
    let mut writer = CsvAccountsReportWriter::new(&mut buffer);

    let report = vec![
      AccountReport::new(1, dec!(100), dec!(10), dec!(110), false),
      AccountReport::new(2, dec!(90), dec!(-10), dec!(80), true),
    ]
    .into_iter();

    let result = writer.write_accounts_report(report).await;

    assert!(result.is_err());
  }

  #[tokio::test]
  async fn write_accounts_empty() {
    let mut buffer = Vec::<u8>::with_capacity(1024);
    let mut writer = CsvAccountsReportWriter::new(&mut buffer);

    let result = writer.write_accounts_report(iter::empty()).await;

    assert!(result.is_ok());
    assert_eq!(String::from_utf8_lossy(buffer.as_slice()), "".to_string())
  }

  #[tokio::test]
  async fn write_accounts_report_success() {
    let mut buffer = Vec::<u8>::with_capacity(1024);
    let mut writer = CsvAccountsReportWriter::new(&mut buffer);

    let report = vec![
      AccountReport::new(1, dec!(100), dec!(10), dec!(110), false),
      AccountReport::new(2, dec!(90), dec!(-10), dec!(80), true),
    ]
    .into_iter();

    let result = writer.write_accounts_report(report).await;

    assert!(result.is_ok());
    assert_eq!(
      String::from_utf8_lossy(buffer.as_slice()),
      "client,available,held,total,locked\n1,100,10,110,false\n2,90,-10,80,true\n".to_string()
    )
  }
}
