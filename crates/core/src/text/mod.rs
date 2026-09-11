pub mod normalize;
pub mod verbatim;

pub use normalize::{fold, is_nfc, nfc};
pub use verbatim::{DerivationRule, Derived, Verbatim};
