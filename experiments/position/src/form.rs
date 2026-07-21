// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Deterministic `form-10k` corpus from ADR-0001.

use std::ops::Range;

use crate::model::{AnchorToken, AuthoredSpan, Bias, CanonicalBaseline, EdgeBehavior};

pub(crate) const FORM_BYTES: usize = 10 * 1024;
pub(crate) const FORM_SPANS: usize = 256;
pub(crate) const FORM_ANCHORS: usize = 64;
pub(crate) const COMPOSITION_START: usize = FORM_BYTES / 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FormEdit {
    pub(crate) replaced: Range<usize>,
    pub(crate) inserted: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FormAnchor {
    pub(crate) token: AnchorToken,
    pub(crate) initial_offset: usize,
    pub(crate) bias: Bias,
}

#[derive(Debug)]
pub(crate) struct FormFixture {
    pub(crate) model: CanonicalBaseline,
    pub(crate) anchors: Vec<FormAnchor>,
}

pub(crate) fn source() -> String {
    "x".repeat(FORM_BYTES)
}

pub(crate) fn authored_spans() -> Vec<AuthoredSpan> {
    let width = FORM_BYTES / FORM_SPANS;
    (0..FORM_SPANS)
        .map(|value| {
            let start = value * width;
            AuthoredSpan {
                range: start..start + width,
                edges: EdgeBehavior {
                    start: if value % 2 == 0 {
                        Bias::Before
                    } else {
                        Bias::After
                    },
                    end: if value % 3 == 0 {
                        Bias::After
                    } else {
                        Bias::Before
                    },
                },
                value: u32::try_from(value).expect("form span count fits in u32"),
            }
        })
        .collect()
}

pub(crate) fn fixture() -> FormFixture {
    let mut model = CanonicalBaseline::new(&source());
    model.replace_authored(authored_spans());
    let mut anchors = Vec::with_capacity(FORM_ANCHORS);

    for value in 0..FORM_ANCHORS - 2 {
        let offset = value * FORM_BYTES / (FORM_ANCHORS - 3);
        let bias = if value % 2 == 0 {
            Bias::Before
        } else {
            Bias::After
        };
        let token = model
            .create_anchor(offset, bias)
            .expect("generated form anchor is valid");
        anchors.push(FormAnchor {
            token,
            initial_offset: offset,
            bias,
        });
    }
    for bias in [Bias::Before, Bias::After] {
        let token = model
            .create_anchor(COMPOSITION_START, bias)
            .expect("composition anchor is valid");
        anchors.push(FormAnchor {
            token,
            initial_offset: COMPOSITION_START,
            bias,
        });
    }

    debug_assert_eq!(
        anchors.len(),
        FORM_ANCHORS,
        "form fixture must contain the ratified anchor count"
    );
    FormFixture { model, anchors }
}

pub(crate) fn ime_edits() -> [FormEdit; 4] {
    [
        FormEdit {
            replaced: COMPOSITION_START..COMPOSITION_START,
            inserted: "n",
        },
        FormEdit {
            replaced: COMPOSITION_START..COMPOSITION_START + 1,
            inserted: "に",
        },
        FormEdit {
            replaced: COMPOSITION_START..COMPOSITION_START + "に".len(),
            inserted: "日本",
        },
        FormEdit {
            replaced: COMPOSITION_START..COMPOSITION_START + "日本".len(),
            inserted: "日本語",
        },
    ]
}

pub(crate) fn expected_anchor_offset(anchor: FormAnchor, composition_bytes: usize) -> usize {
    match anchor.initial_offset.cmp(&COMPOSITION_START) {
        std::cmp::Ordering::Less => anchor.initial_offset,
        std::cmp::Ordering::Equal => match anchor.bias {
            Bias::Before => COMPOSITION_START,
            Bias::After => COMPOSITION_START + composition_bytes,
        },
        std::cmp::Ordering::Greater => anchor.initial_offset + composition_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::{COMPOSITION_START, FORM_ANCHORS, FORM_BYTES, FORM_SPANS, fixture, ime_edits};

    #[test]
    fn corpus_has_the_ratified_scale() {
        let fixture = fixture();
        assert_eq!(fixture.model.text_len(), FORM_BYTES);
        assert_eq!(fixture.model.authored().len(), FORM_SPANS);
        assert_eq!(fixture.anchors.len(), FORM_ANCHORS);
    }

    #[test]
    fn ime_sequence_replaces_one_active_composition_range() {
        let mut fixture = fixture();
        for edit in ime_edits() {
            fixture
                .model
                .replace(edit.replaced, edit.inserted)
                .expect("IME edit is valid");
        }
        let (snapshot, _) = fixture.model.snapshot();
        assert_eq!(snapshot.text().len(), FORM_BYTES + "日本語".len());
        assert_eq!(
            &snapshot.text()[COMPOSITION_START..COMPOSITION_START + "日本語".len()],
            "日本語"
        );
    }
}
