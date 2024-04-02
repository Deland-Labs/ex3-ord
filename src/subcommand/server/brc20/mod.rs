use super::{types::ScriptPubkey, *};
mod balance;
mod inscribe_brc20_transferable;
mod receipt;
mod ticker;
mod transferable;

#[derive(Debug, thiserror::Error)]
pub(super) enum BRC20Error {
  #[error("ticker must be 4 bytes length")]
  IncorrectTickFormat,
  #[error("tick not found")]
  TickNotFound,
  #[error("balance not found")]
  BalanceNotFound,
  #[error("events not found")]
  EventsNotFound,
  #[error("block not found")]
  BlockNotFound,
}

pub(super) use {
  balance::*, inscribe_brc20_transferable::*, receipt::*, ticker::*, transferable::*,
};
