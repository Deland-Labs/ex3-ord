use super::*;

mod inscription;
mod outpoint;
mod transaction;

pub(super) use {inscription::*, outpoint::*, transaction::*};

#[derive(Debug, thiserror::Error)]
pub enum OrdApiError {
  /// Thrown when a inscription id was requested but not matching inscription exists
  #[error("unknown inscription id {0}")]
  UnknownInscriptionId(InscriptionId),
  /// Thrown when a inscription number was requested but not matching inscription exists
  #[error("unknown inscription number {0}")]
  UnknownInscriptionNumber(i32),
  /// Thrown when a transaction was requested but not matching transaction exists
  #[error("transaction {0} not found")]
  TransactionNotFound(Txid),
  /// Thrown when a transaction receipt was requested but not matching transaction receipt exists
  #[error("transaction receipt {0} not found")]
  TransactionReceiptNotFound(Txid),
  /// Thrown when parsing the inscription from the transaction fails
  #[error("invalid inscription {0}")]
  InvalidInscription(InscriptionId),
  /// Thrown when the satpoint for the inscription cannot be found
  #[error("satpoint not found for inscription {0}")]
  SatPointNotFound(InscriptionId),
  /// Thrown when an internal error occurs
  #[error("internal error: {0}")]
  Internal(String),
}

impl From<OrdApiError> for ApiError {
  fn from(error: OrdApiError) -> Self {
    match error {
      OrdApiError::UnknownInscriptionId(_) => Self::not_found(error.to_string()),
      OrdApiError::UnknownInscriptionNumber(_) => Self::not_found(error.to_string()),
      OrdApiError::TransactionReceiptNotFound(_) => Self::not_found(error.to_string()),
      OrdApiError::TransactionNotFound(_) => Self::not_found(error.to_string()),
      OrdApiError::InvalidInscription(_) => Self::internal(error.to_string()),
      OrdApiError::SatPointNotFound(_) => Self::internal(error.to_string()),
      OrdApiError::Internal(_) => Self::internal(error.to_string()),
    }
  }
}
