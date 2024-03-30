use crate::subcommand::wallet::inscribe::InscribeBrc20Transferable;
use {super::*, utoipa::ToSchema};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InscribeBrc20TransferableRequest {
  pub from_wallet: String,
  pub commit_fee_rate: Option<f64>,
  pub tick: String,
  pub amount: f64,
}

/// Inscribe BRC20 transferable
///
/// Inscribe BRC20 transferable to the address.
#[utoipa::path(
post,
path = "/api/v1/brc20/inscribe_transferable",
request_body = InscribeBrc20TransferableRequest,
responses(
(status = 200, description = "Response json", body = String),
(status = 400, description = "Bad query.", body = ApiError, example = json!(&ApiError::bad_request("bad request"))),
(status = 404, description = "Not found.", body = ApiError, example = json!(&ApiError::not_found("not found"))),
(status = 500, description = "Internal server error.", body = ApiError, example = json!(&ApiError::internal("internal error"))),
)
)]
pub(crate) async fn inscribe_brc20_transferable(
  Extension(index): Extension<Arc<Index>>,
  Json(req): Json<InscribeBrc20TransferableRequest>,
) -> ApiResult<String> {
  let cmd = InscribeBrc20Transferable {
    from_wallet: req.from_wallet.clone(),
    tick: req.tick.clone(),
    amount: req.amount,
    commit_fee_rate: req.commit_fee_rate.map(|r| FeeRate::try_from(r).unwrap()),
    meta_protocol: None,
    no_backup: false,
  };

  let res = cmd.execute(index);
  match res {
    Ok(r) => Ok(Json(ApiResponse::ok(r.to_json()))),
    Err(e) => {
      return Err(ApiError::internal(e.to_string()));
    }
  }
}
