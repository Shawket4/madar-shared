//! The refusals a plan gives, with the one wording every app shows.

use serde::{Deserialize, Serialize};

use crate::Feature;

/// Why a plan said no. The strings are the API's error codes.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub enum Code {
    /// The plan lacks a feature (`feature` says which).
    #[serde(rename = "plan_feature")]
    Feature,
    /// The plan's branches are used up.
    #[serde(rename = "plan_branch_limit")]
    BranchLimit,
    /// The branch's devices of a kind are used up.
    #[serde(rename = "plan_device_limit")]
    DeviceLimit,
    /// The plan's tills hold no realtime stream (`403 plan_no_realtime`).
    #[serde(rename = "plan_no_realtime")]
    NoRealtime,
    /// A downgraded organisation's till that is not its branch's active one.
    #[serde(rename = "plan_till_inactive")]
    TillInactive,
}

impl Code {
    pub fn as_str(self) -> &'static str {
        match self {
            Code::Feature => "plan_feature",
            Code::BranchLimit => "plan_branch_limit",
            Code::DeviceLimit => "plan_device_limit",
            Code::NoRealtime => "plan_no_realtime",
            Code::TillInactive => "plan_till_inactive",
        }
    }

    pub fn parse(s: &str) -> Option<Code> {
        [
            Code::Feature,
            Code::BranchLimit,
            Code::DeviceLimit,
            Code::NoRealtime,
            Code::TillInactive,
        ]
        .into_iter()
        .find(|c| c.as_str() == s)
    }
}

/// A plan's refusal: the code, and the feature it is about (when it is about
/// one).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
pub struct Refusal {
    pub code: Code,
    pub feature: Option<Feature>,
}

impl Refusal {
    pub fn new(code: Code, feature: Option<Feature>) -> Refusal {
        Refusal { code, feature }
    }

    /// The plan lacks `feature`.
    pub fn feature(feature: Feature) -> Refusal {
        Refusal::new(Code::Feature, Some(feature))
    }

    /// The message in English.
    pub fn en(&self) -> &'static str {
        match (self.code, self.feature) {
            (Code::Feature | Code::DeviceLimit, Some(f)) => match f {
                Feature::Inventory => "Inventory and purchasing are on the paid plan.",
                Feature::Loyalty => "Loyalty is on the paid plan.",
                Feature::Online => "Online ordering and delivery are on the paid plan.",
                Feature::Bookings => "Bookings are on the paid plan.",
                Feature::Floor => "Tables and open tickets are on the paid plan.",
                Feature::Kitchen => "Kitchen screens are on the paid plan.",
                Feature::Dawam => "Staff scheduling and payroll are on the paid plan.",
                Feature::Branding => "Custom branding and your own web address are on the paid plan.",
                Feature::Basira => "Basira is on the paid plan.",
            },
            (Code::Feature, None) => "This is on the paid plan.",
            (Code::BranchLimit, _) => {
                "Your plan has reached its limit of branches. Upgrade to add more."
            }
            (Code::DeviceLimit, None) => {
                "This branch has reached its limit of POS devices on your plan. Upgrade to add more."
            }
            (Code::NoRealtime, _) => "Live updates are on the paid plan.",
            (Code::TillInactive, _) => {
                "This till isn't active on the Free plan. The owner can choose the active till in the dashboard, or upgrade."
            }
        }
    }

    /// The message in Modern Standard Arabic.
    pub fn ar(&self) -> &'static str {
        match (self.code, self.feature) {
            (Code::Feature | Code::DeviceLimit, Some(f)) => match f {
                Feature::Inventory => "المخزون والمشتريات متاحان في الخطة المدفوعة.",
                Feature::Loyalty => "برنامج الولاء متاح في الخطة المدفوعة.",
                Feature::Online => "الطلب عبر الإنترنت والتوصيل متاحان في الخطة المدفوعة.",
                Feature::Bookings => "الحجوزات متاحة في الخطة المدفوعة.",
                Feature::Floor => "الطاولات والفواتير المفتوحة متاحة في الخطة المدفوعة.",
                Feature::Kitchen => "شاشات المطبخ متاحة في الخطة المدفوعة.",
                Feature::Dawam => "جداول الموظفين والرواتب متاحة في الخطة المدفوعة.",
                Feature::Branding => "الهوية المخصصة والعنوان الإلكتروني الخاص متاحان في الخطة المدفوعة.",
                Feature::Basira => "بصيرة متاحة في الخطة المدفوعة.",
            },
            (Code::Feature, None) => "هذه الميزة متاحة في الخطة المدفوعة.",
            (Code::BranchLimit, _) => "بلغت خطتك الحد الأقصى للفروع. قم بترقية الخطة لإضافة المزيد.",
            (Code::DeviceLimit, None) => {
                "بلغ هذا الفرع الحد الأقصى لأجهزة نقطة البيع في خطتك. قم بترقية الخطة لإضافة المزيد."
            }
            (Code::NoRealtime, _) => "التحديث المباشر متاح في الخطة المدفوعة.",
            (Code::TillInactive, _) => {
                "هذا الجهاز غير مفعّل في الخطة المجانية. يمكن للمالك اختيار الجهاز المفعّل من لوحة التحكم، أو ترقية الخطة."
            }
        }
    }

    /// The message for a locale: `ar` (or `ar-EG`, …) is Arabic, anything else English.
    pub fn message(&self, locale: &str) -> &'static str {
        let lang = locale.split(['-', '_']).next().unwrap_or("");
        if lang.eq_ignore_ascii_case("ar") {
            self.ar()
        } else {
            self.en()
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.en())
    }
}

impl std::error::Error for Refusal {}
