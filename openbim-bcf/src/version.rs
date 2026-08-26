//! BCF-XML container versions and how they are detected.

use openbim_core::Detected;

/// A BCF-XML container version.
///
/// These are not cosmetic revisions. Each step changes where information
/// lives, so a reader that picks the wrong one does not fail — it silently
/// yields a different document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BcfVersion {
    /// 2.0 — `TopicStatus`/`TopicType` are child elements of `Topic`, and each
    /// `Comment` nests a back-reference `Topic` element.
    V2_0,
    /// 2.1 — status and type move onto `Topic` attributes; the comment
    /// back-reference is dropped.
    V2_1,
    /// 3.0 — viewpoints move into a `Viewpoints` wrapper element, topics gain
    /// `ServerAssignedId`, and `Header` gains `File/@IsExternal` semantics.
    V3_0,
}

impl BcfVersion {
    /// Every version this crate recognises, oldest first.
    pub const ALL: [BcfVersion; 3] = [BcfVersion::V2_0, BcfVersion::V2_1, BcfVersion::V3_0];

    /// Whether documents of this version nest a back-reference `Topic` element
    /// inside each comment.
    ///
    /// 2.0's `Comment` type requires a `Topic` child pointing back at its
    /// owner; 2.1 dropped it. A reader that does not expect this mistakes the
    /// back-reference for a second topic declaration. Measured: 106 of 121
    /// comments in the third-party corpus carry one.
    #[must_use]
    pub fn nests_topic_in_comment(self) -> bool {
        matches!(self, BcfVersion::V2_0)
    }

    /// Whether comments carry the 2.0-only `Status` and `VerbalStatus`
    /// elements and a `ReplyToComment` reference.
    ///
    /// All three were removed in 2.1. Together with the back-reference they
    /// are the reliable 2.0 markers — **not** the location of `TopicStatus`,
    /// which is a `Topic` attribute in 2.0 and 2.1 alike.
    #[must_use]
    pub fn comments_carry_status(self) -> bool {
        matches!(self, BcfVersion::V2_0)
    }

    /// Whether markup nests topic collections (`Comments`, `Viewpoints`,
    /// `Labels/Label`, `DocumentReferences/DocumentReference`) inside `Topic`.
    ///
    /// 3.0 moved them there; in 2.x `Comment` and `Viewpoints` are siblings of
    /// `Topic` under `Markup`.
    #[must_use]
    pub fn nests_collections_in_topic(self) -> bool {
        matches!(self, BcfVersion::V3_0)
    }

    /// The `VersionId` string this version writes into `bcf.version`.
    #[must_use]
    pub fn version_id(self) -> &'static str {
        match self {
            BcfVersion::V2_0 => "2.0",
            BcfVersion::V2_1 => "2.1",
            BcfVersion::V3_0 => "3.0",
        }
    }

    /// Parse a `bcf.version/@VersionId` value.
    ///
    /// Accepts the exact published identifiers and the common `x.y.z` form
    /// some writers emit (`"3.0.0"`). Unknown values return `None` rather than
    /// being rounded to a neighbour: an unrecognised version is a fact worth
    /// reporting, not a default worth guessing.
    #[must_use]
    pub fn from_version_id(raw: &str) -> Option<BcfVersion> {
        match raw.trim() {
            "2.0" | "2.0.0" => Some(BcfVersion::V2_0),
            "2.1" | "2.1.0" => Some(BcfVersion::V2_1),
            "3.0" | "3.0.0" => Some(BcfVersion::V3_0),
            _ => None,
        }
    }

    /// Reconcile a declared version with one inferred from document shape.
    ///
    /// Wraps [`Detected`] so callers see *how* the version was established.
    /// Disagreement is surfaced, never resolved here: which side is right is a
    /// caller policy decision.
    #[must_use]
    pub fn reconcile(
        declared: Option<BcfVersion>,
        observed: Option<BcfVersion>,
    ) -> Detected<BcfVersion> {
        match (declared, observed) {
            (Some(d), Some(o)) if d == o => Detected::Declared(d),
            (Some(declared), Some(observed)) => Detected::Conflict { declared, observed },
            (Some(d), None) => Detected::Declared(d),
            (None, Some(o)) => Detected::Inferred(o),
            // Nothing declared and nothing distinguishing observed. 2.1 is the
            // most widespread interchange version and the only one whose shape
            // is a subset of both neighbours for the fields we read.
            (None, None) => Detected::Inferred(BcfVersion::V2_1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_2_0_nests_topic_backreferences() {
        assert!(BcfVersion::V2_0.nests_topic_in_comment());
        assert!(!BcfVersion::V2_1.nests_topic_in_comment());
        assert!(!BcfVersion::V3_0.nests_topic_in_comment());
    }

    #[test]
    fn only_2_0_keeps_status_on_comments() {
        assert!(BcfVersion::V2_0.comments_carry_status());
        assert!(!BcfVersion::V2_1.comments_carry_status());
        assert!(!BcfVersion::V3_0.comments_carry_status());
    }

    #[test]
    fn only_3_0_nests_collections_in_topic() {
        assert!(!BcfVersion::V2_0.nests_collections_in_topic());
        assert!(!BcfVersion::V2_1.nests_collections_in_topic());
        assert!(BcfVersion::V3_0.nests_collections_in_topic());
    }

    #[test]
    fn versions_order_oldest_first() {
        assert!(BcfVersion::V2_0 < BcfVersion::V2_1);
        assert!(BcfVersion::V2_1 < BcfVersion::V3_0);
    }

    #[test]
    fn version_ids_round_trip() {
        for v in BcfVersion::ALL {
            assert_eq!(BcfVersion::from_version_id(v.version_id()), Some(v));
        }
    }

    #[test]
    fn patch_suffixed_ids_are_accepted_but_unknown_ones_are_not() {
        assert_eq!(BcfVersion::from_version_id("3.0.0"), Some(BcfVersion::V3_0));
        assert_eq!(BcfVersion::from_version_id(" 2.1 "), Some(BcfVersion::V2_1));
        assert_eq!(BcfVersion::from_version_id("1.0"), None);
        assert_eq!(BcfVersion::from_version_id("4.0"), None);
        assert_eq!(BcfVersion::from_version_id(""), None);
    }

    #[test]
    fn declared_and_observed_disagreement_is_a_conflict() {
        assert_eq!(
            BcfVersion::reconcile(Some(BcfVersion::V2_1), Some(BcfVersion::V2_0)),
            Detected::Conflict {
                declared: BcfVersion::V2_1,
                observed: BcfVersion::V2_0
            }
        );
    }

    #[test]
    fn agreement_and_single_sided_evidence_resolve() {
        assert_eq!(
            BcfVersion::reconcile(Some(BcfVersion::V3_0), Some(BcfVersion::V3_0)),
            Detected::Declared(BcfVersion::V3_0)
        );
        assert_eq!(
            BcfVersion::reconcile(Some(BcfVersion::V2_0), None),
            Detected::Declared(BcfVersion::V2_0)
        );
        assert_eq!(
            BcfVersion::reconcile(None, Some(BcfVersion::V2_0)),
            Detected::Inferred(BcfVersion::V2_0)
        );
    }

    #[test]
    fn absent_evidence_infers_rather_than_declares() {
        let detected = BcfVersion::reconcile(None, None);
        assert_eq!(detected, Detected::Inferred(BcfVersion::V2_1));
        // The point of Inferred: the caller can tell this was a guess.
        assert!(matches!(detected, Detected::Inferred(_)));
    }
}
