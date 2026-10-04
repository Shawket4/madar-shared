//! The request and response bodies of the warehouse and transfer endpoints
//! (WAREHOUSE_DESIGN.md §7). Quantities are in the ingredient's base stock
//! unit; costs are piastres per base unit — fractional, as a gram of coffee
//! costs less than a piastre — and `None` = unknown (never 0).
//!
//! With the `utoipa` feature each type is also the backend's OpenAPI schema.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::transfer::TransferStatus;

/// What a `branches` row is. A warehouse holds stock and never sells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum BranchKind {
    #[default]
    Branch,
    Warehouse,
}

impl BranchKind {
    /// The Postgres `branch_kind` label.
    pub fn as_str(self) -> &'static str {
        match self {
            BranchKind::Branch => "branch",
            BranchKind::Warehouse => "warehouse",
        }
    }
}

impl std::str::FromStr for BranchKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "branch" => Ok(BranchKind::Branch),
            "warehouse" => Ok(BranchKind::Warehouse),
            other => Err(format!("unknown branch kind '{other}'")),
        }
    }
}

/// So a backend can read the `branch_kind` column as text.
impl TryFrom<String> for BranchKind {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

/// One ingredient and how much of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TransferLineInput {
    pub org_ingredient_id: Uuid,
    /// Greater than 0.
    pub quantity: f64,
    #[serde(default)]
    pub note: Option<String>,
}

/// `POST /inventory/transfers`. With `request: true` the DESTINATION asks
/// (status `requested`); otherwise the SOURCE starts a `draft`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateTransferRequest {
    pub source_branch_id: Uuid,
    pub destination_branch_id: Uuid,
    #[serde(default)]
    pub request: bool,
    /// At least one; each ingredient once.
    pub lines: Vec<TransferLineInput>,
    #[serde(default)]
    pub note: Option<String>,
}

/// `PATCH /inventory/transfers/{id}`. `lines`, when given, replaces every
/// line (only while `requested` or `draft`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateTransferRequest {
    #[serde(default)]
    pub lines: Option<Vec<TransferLineInput>>,
    #[serde(default)]
    pub note: Option<String>,
}

/// `POST /inventory/transfers/{id}/accept`: a request becomes the source's
/// draft, optionally with its lines changed.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AcceptTransferRequest {
    #[serde(default)]
    pub lines: Option<Vec<TransferLineInput>>,
}

/// `POST /inventory/transfers/{id}/decline` (note required) and
/// `POST /inventory/transfers/{id}/cancel` (note optional).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CloseTransferRequest {
    #[serde(default)]
    pub note: Option<String>,
}

/// One line as it arrived.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ReceiveTransferLine {
    pub line_id: Uuid,
    /// 0 or more. More than sent needs `note`.
    pub qty_received: f64,
    #[serde(default)]
    pub note: Option<String>,
}

/// `POST /inventory/transfers/{id}/receive`: every line, once. Closes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ReceiveTransferRequest {
    pub lines: Vec<ReceiveTransferLine>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct StockTransferLine {
    pub id: Uuid,
    pub org_ingredient_id: Uuid,
    pub ingredient_name: String,
    pub unit: String,
    /// Asked for while `requested`, planned while `draft`, sent from dispatch on.
    pub qty_sent: f64,
    /// `None` until received.
    pub qty_received: Option<f64>,
    /// Frozen at dispatch from the source's cost; `None` before dispatch or unknown.
    pub unit_cost: Option<f64>,
    pub note: Option<String>,
}

/// Who did a step and when.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TransferStamp {
    pub at: DateTime<Utc>,
    pub by: Uuid,
    pub by_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct StockTransfer {
    pub id: Uuid,
    pub org_id: Uuid,
    /// `TR-1043`, per org.
    pub reference: String,
    pub status: TransferStatus,
    pub source_branch_id: Uuid,
    pub source_branch_name: String,
    pub source_kind: BranchKind,
    pub destination_branch_id: Uuid,
    pub destination_branch_name: String,
    pub destination_kind: BranchKind,
    pub note: Option<String>,
    pub lines: Vec<StockTransferLine>,
    pub created: TransferStamp,
    pub requested: Option<TransferStamp>,
    pub dispatched: Option<TransferStamp>,
    pub received: Option<TransferStamp>,
    pub cancelled: Option<TransferStamp>,
}

/// `GET /inventory/warehouses/{id}/replenishment?branch_id=`: one row per
/// ingredient the branch is at or under its low-stock level on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ReplenishmentRow {
    pub org_ingredient_id: Uuid,
    pub ingredient_name: String,
    pub unit: String,
    pub category_name: String,
    pub on_hand: f64,
    pub par_min: f64,
    pub par_max: Option<f64>,
    pub in_transit: f64,
    pub open_inbound: f64,
    pub warehouse_on_hand: f64,
    /// See [`crate::replenish::Suggestion`].
    pub need: f64,
    pub available: f64,
    pub suggested: f64,
}

/// `GET /reports/orgs/{org}/transfer-differences`: one received line whose
/// received quantity differs from what was sent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TransferDifferenceRow {
    pub transfer_id: Uuid,
    pub reference: String,
    pub received_at: DateTime<Utc>,
    pub source_branch_name: String,
    pub destination_branch_name: String,
    pub org_ingredient_id: Uuid,
    pub ingredient_name: String,
    pub unit: String,
    pub qty_sent: f64,
    pub qty_received: f64,
    /// received − sent; negative = transit loss.
    pub difference: f64,
    pub unit_cost: Option<f64>,
    /// `difference × unit_cost`, piastres; `None` when the cost is unknown.
    pub value_difference: Option<i64>,
    pub note: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_kind_round_trips_its_label() {
        for k in [BranchKind::Branch, BranchKind::Warehouse] {
            assert_eq!(BranchKind::try_from(k.as_str().to_string()), Ok(k));
            assert_eq!(serde_json::to_value(k).unwrap(), k.as_str());
        }
        assert!("shop".parse::<BranchKind>().is_err());
    }
}
