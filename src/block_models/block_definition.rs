//! Blockstate definitions: which models each state of a block draws.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::model_json::asset_stem;
use crate::constants::PROPERTY_VALUE_SETS;

/// One `assets/{namespace}/blockstates/{block}.json`, reduced to what the
/// editor uses. Model ids are stems; weighted model lists keep their first
/// entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockDefinition {
    /// Each state draws exactly one model.
    Variants(Vec<Variant>),
    /// Each state draws every case whose condition holds.
    Multipart(Vec<Case>),
}

/// A model placed by a blockstate, with its rotation in degrees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppliedModel {
    pub model: String,
    #[serde(default)]
    pub x: u16,
    #[serde(default)]
    pub y: u16,
    #[serde(default)]
    pub uvlock: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Variant {
    /// The property values this variant is for. Empty for a block with a
    /// single state.
    pub properties: BTreeMap<String, String>,
    pub apply: AppliedModel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Case {
    /// `None` applies to every state.
    pub when: Option<Condition>,
    pub apply: AppliedModel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Condition {
    /// Every listed property has one of its listed values.
    Properties(BTreeMap<String, Vec<String>>),
    Or(Vec<Condition>),
    And(Vec<Condition>),
}

impl AppliedModel {
    /// `model` with no rotation.
    pub fn unrotated(model: String) -> Self {
        Self {
            model,
            x: 0,
            y: 0,
            uvlock: false,
        }
    }

    fn is_unrotated(&self) -> bool {
        self.x.is_multiple_of(360) && self.y.is_multiple_of(360)
    }
}

impl Condition {
    fn holds(&self, properties: &BTreeMap<String, String>) -> bool {
        match self {
            Self::Properties(wanted) => wanted.iter().all(|(name, values)| {
                properties
                    .get(name)
                    .is_some_and(|value| values.contains(value))
            }),
            Self::Or(any) => any.iter().any(|condition| condition.holds(properties)),
            Self::And(all) => all.iter().all(|condition| condition.holds(properties)),
        }
    }

    fn collect_values(&self, into: &mut BTreeMap<String, BTreeSet<String>>) {
        match self {
            Self::Properties(wanted) => {
                for (name, values) in wanted {
                    into.entry(name.clone())
                        .or_default()
                        .extend(values.iter().cloned());
                }
            }
            Self::Or(list) | Self::And(list) => {
                for condition in list {
                    condition.collect_values(into);
                }
            }
        }
    }
}

impl BlockDefinition {
    /// Parses a blockstate JSON from a jar.
    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        let raw: RawFile = serde_json::from_slice(bytes).map_err(|err| err.to_string())?;
        if let Some(variants) = raw.variants {
            let variants = variants
                .into_iter()
                .filter_map(|(key, apply)| {
                    Some(Variant {
                        properties: parse_variant_key(&key),
                        apply: apply.first()?,
                    })
                })
                .collect();
            return Ok(Self::Variants(variants));
        }
        let Some(multipart) = raw.multipart else {
            return Err("neither `variants` nor `multipart`".to_owned());
        };
        let mut cases = Vec::new();
        for case in multipart {
            let when = match &case.when {
                Some(value) => Some(parse_condition(value).ok_or("malformed `when`")?),
                None => None,
            };
            if let Some(apply) = case.apply.first() {
                cases.push(Case { when, apply });
            }
        }
        Ok(Self::Multipart(cases))
    }

    /// The models the state with `properties` draws.
    ///
    /// A `Variants` state that names no variant exactly (for example one
    /// with no properties at all) takes the first variant it does not
    /// contradict, preferring an unrotated one.
    pub fn select(&self, properties: &BTreeMap<String, String>) -> Vec<&AppliedModel> {
        match self {
            Self::Variants(variants) => self
                .variant_index(properties)
                .map(|index| &variants[index].apply)
                .into_iter()
                .collect(),
            Self::Multipart(cases) => cases
                .iter()
                .filter(|case| case.when.as_ref().is_none_or(|when| when.holds(properties)))
                .map(|case| &case.apply)
                .collect(),
        }
    }

    /// Whether any variant or condition reads `property`.
    pub fn mentions(&self, property: &str) -> bool {
        match self {
            Self::Variants(variants) => variants
                .iter()
                .any(|variant| variant.properties.contains_key(property)),
            Self::Multipart(_) => self.multipart_values().contains_key(property),
        }
    }

    /// Every state the block can be placed in, as property maps, at most
    /// `limit` of them.
    ///
    /// A multipart block lists every combination of the property values its
    /// conditions test.
    pub fn variants(&self, limit: usize) -> Vec<BTreeMap<String, String>> {
        match self {
            Self::Variants(variants) => variants
                .iter()
                .take(limit)
                .map(|variant| variant.properties.clone())
                .collect(),
            Self::Multipart(_) => {
                let axes: Vec<(String, Vec<String>)> = self
                    .multipart_values()
                    .into_iter()
                    .map(|(name, seen)| (name, complete_values(seen)))
                    .collect();
                let mut combinations = vec![BTreeMap::new()];
                for (name, values) in &axes {
                    combinations = combinations
                        .iter()
                        .flat_map(|base| {
                            values.iter().map(move |value| {
                                let mut next = base.clone();
                                next.insert(name.clone(), value.clone());
                                next
                            })
                        })
                        .take(limit)
                        .collect();
                }
                combinations
            }
        }
    }

    /// Index into `variants` (from [`Self::variants`]) of the state a newly
    /// picked block starts in: an unrotated variant, or the first multipart
    /// combination that draws anything.
    pub fn default_variant(&self, variants: &[BTreeMap<String, String>]) -> usize {
        match self {
            Self::Variants(_) => self
                .variant_index(&BTreeMap::new())
                .filter(|&index| index < variants.len())
                .unwrap_or(0),
            Self::Multipart(_) => variants
                .iter()
                .position(|properties| !self.select(properties).is_empty())
                .unwrap_or(0),
        }
    }

    fn variant_index(&self, properties: &BTreeMap<String, String>) -> Option<usize> {
        let Self::Variants(variants) = self else {
            return None;
        };
        let exact = variants.iter().position(|variant| {
            variant
                .properties
                .iter()
                .all(|(name, value)| properties.get(name) == Some(value))
        });
        if exact.is_some() {
            return exact;
        }
        let compatible = |variant: &Variant| {
            variant
                .properties
                .iter()
                .all(|(name, value)| properties.get(name).is_none_or(|given| given == value))
        };
        variants
            .iter()
            .position(|variant| compatible(variant) && variant.apply.is_unrotated())
            .or_else(|| variants.iter().position(compatible))
            .or(if variants.is_empty() { None } else { Some(0) })
    }

    fn multipart_values(&self) -> BTreeMap<String, BTreeSet<String>> {
        let mut values = BTreeMap::new();
        if let Self::Multipart(cases) = self {
            for case in cases {
                if let Some(when) = &case.when {
                    when.collect_values(&mut values);
                }
            }
        }
        values
    }
}

/// The values a multipart property can take, given the ones its conditions
/// test. Conditions often leave the "off" value out (a fence only tests
/// `north=true`), so a known value set is filled in whole.
fn complete_values(seen: BTreeSet<String>) -> Vec<String> {
    PROPERTY_VALUE_SETS
        .iter()
        .find(|set| seen.iter().all(|value| set.contains(&value.as_str())))
        .map(|set| set.iter().map(|&value| value.to_owned()).collect())
        .unwrap_or_else(|| seen.into_iter().collect())
}

/// `facing=north,half=top` -> `{facing: north, half: top}`.
fn parse_variant_key(key: &str) -> BTreeMap<String, String> {
    key.split(',')
        .filter_map(|pair| pair.split_once('='))
        .map(|(name, value)| (name.trim().to_owned(), value.trim().to_owned()))
        .collect()
}

fn parse_condition(value: &serde_json::Value) -> Option<Condition> {
    let object = value.as_object()?;
    let list = |key: &str| -> Option<Option<Vec<Condition>>> {
        object
            .get(key)
            .map(|items| items.as_array()?.iter().map(parse_condition).collect())
    };
    if let Some(any) = list("OR") {
        return any.map(Condition::Or);
    }
    if let Some(all) = list("AND") {
        return all.map(Condition::And);
    }
    Some(Condition::Properties(
        object
            .iter()
            .map(|(name, value)| {
                let text = match value {
                    serde_json::Value::String(text) => text.clone(),
                    other => other.to_string(),
                };
                (
                    name.clone(),
                    text.split('|').map(|value| value.to_owned()).collect(),
                )
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
struct RawFile {
    #[serde(default)]
    variants: Option<BTreeMap<String, RawApply>>,
    #[serde(default)]
    multipart: Option<Vec<RawCase>>,
}

#[derive(Deserialize)]
struct RawCase {
    #[serde(default)]
    when: Option<serde_json::Value>,
    apply: RawApply,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawApply {
    One(RawModel),
    Many(Vec<RawModel>),
}

#[derive(Deserialize)]
struct RawModel {
    model: String,
    #[serde(default)]
    x: u16,
    #[serde(default)]
    y: u16,
    #[serde(default)]
    uvlock: bool,
}

impl RawApply {
    fn first(self) -> Option<AppliedModel> {
        let raw = match self {
            Self::One(raw) => raw,
            Self::Many(list) => list.into_iter().next()?,
        };
        Some(AppliedModel {
            model: asset_stem(&raw.model),
            x: raw.x,
            y: raw.y,
            uvlock: raw.uvlock,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|&(name, value)| (name.to_owned(), value.to_owned()))
            .collect()
    }

    fn models(definition: &BlockDefinition, pairs: &[(&str, &str)]) -> Vec<String> {
        definition
            .select(&props(pairs))
            .into_iter()
            .map(|applied| applied.model.clone())
            .collect()
    }

    fn log() -> BlockDefinition {
        BlockDefinition::from_json(
            br#"{"variants": {
                "axis=x": {"model": "minecraft:block/oak_log_horizontal", "x": 90, "y": 90},
                "axis=y": {"model": "minecraft:block/oak_log"},
                "axis=z": {"model": "minecraft:block/oak_log_horizontal", "x": 90}
            }}"#,
        )
        .unwrap()
    }

    fn fence() -> BlockDefinition {
        BlockDefinition::from_json(
            br#"{"multipart": [
                {"apply": {"model": "minecraft:block/oak_fence_post"}},
                {"when": {"north": "true"},
                 "apply": {"model": "minecraft:block/oak_fence_side", "uvlock": true}},
                {"when": {"east": "true"},
                 "apply": {"model": "minecraft:block/oak_fence_side", "y": 90, "uvlock": true}}
            ]}"#,
        )
        .unwrap()
    }

    fn wire() -> BlockDefinition {
        BlockDefinition::from_json(
            br#"{"multipart": [
                {"when": {"OR": [
                    {"east": "none", "north": "none"},
                    {"east": "side|up", "north": "side|up"}
                 ]},
                 "apply": {"model": "minecraft:block/redstone_dust_dot"}},
                {"when": {"north": "side|up"},
                 "apply": {"model": "minecraft:block/redstone_dust_side0"}}
            ]}"#,
        )
        .unwrap()
    }

    #[test]
    fn variant_is_chosen_by_its_properties() {
        let log = log();
        assert_eq!(models(&log, &[("axis", "y")]), ["oak_log"]);
        let applied = log.select(&props(&[("axis", "x")]))[0];
        assert_eq!(
            (applied.model.as_str(), applied.x, applied.y),
            ("oak_log_horizontal", 90, 90)
        );
    }

    #[test]
    fn state_without_properties_takes_an_unrotated_variant() {
        let log = log();
        assert_eq!(models(&log, &[]), ["oak_log"]);
        let variants = log.variants(100);
        assert_eq!(variants.len(), 3);
        assert_eq!(
            variants[log.default_variant(&variants)],
            props(&[("axis", "y")])
        );
    }

    #[test]
    fn weighted_model_list_keeps_its_first_entry() {
        let bamboo = BlockDefinition::from_json(
            br#"{"multipart": [{"when": {"age": "0"}, "apply": [
                {"model": "minecraft:block/bamboo1_age0"},
                {"model": "minecraft:block/bamboo2_age0"}
            ]}]}"#,
        )
        .unwrap();
        assert_eq!(models(&bamboo, &[("age", "0")]), ["bamboo1_age0"]);
        assert!(models(&bamboo, &[("age", "1")]).is_empty());
    }

    #[test]
    fn multipart_draws_every_case_that_holds() {
        let fence = fence();
        assert_eq!(models(&fence, &[]), ["oak_fence_post"]);
        assert_eq!(
            models(&fence, &[("north", "true"), ("east", "true")]),
            ["oak_fence_post", "oak_fence_side", "oak_fence_side"]
        );
    }

    #[test]
    fn or_conditions_and_value_alternatives_are_matched() {
        let wire = wire();
        assert_eq!(
            models(&wire, &[("east", "none"), ("north", "none")]),
            ["redstone_dust_dot"]
        );
        assert_eq!(
            models(&wire, &[("east", "none"), ("north", "up")]),
            ["redstone_dust_side0"]
        );
        assert_eq!(
            models(&wire, &[("east", "side"), ("north", "up")]),
            ["redstone_dust_dot", "redstone_dust_side0"]
        );
    }

    #[test]
    fn multipart_variants_combine_completed_property_values() {
        let variants = fence().variants(100);
        assert_eq!(
            variants,
            [
                props(&[("east", "false"), ("north", "false")]),
                props(&[("east", "false"), ("north", "true")]),
                props(&[("east", "true"), ("north", "false")]),
                props(&[("east", "true"), ("north", "true")]),
            ]
        );
        assert_eq!(fence().default_variant(&variants), 0);
        assert_eq!(wire().variants(100).len(), 9);
        assert_eq!(wire().variants(4).len(), 4);
    }

    #[test]
    fn wall_heights_gain_the_untested_none() {
        let wall = BlockDefinition::from_json(
            br#"{"multipart": [
                {"when": {"east": "low"}, "apply": {"model": "wall_side"}},
                {"when": {"east": "tall"}, "apply": {"model": "wall_side_tall"}}
            ]}"#,
        )
        .unwrap();
        let variants = wall.variants(100);
        assert_eq!(
            variants,
            [
                props(&[("east", "none")]),
                props(&[("east", "low")]),
                props(&[("east", "tall")]),
            ]
        );
        // `none` draws nothing, so the block starts on the first state that does.
        assert_eq!(wall.default_variant(&variants), 1);
    }

    #[test]
    fn mentions_reports_properties_the_definition_reads() {
        assert!(log().mentions("axis"));
        assert!(!log().mentions("facing"));
        assert!(wire().mentions("north"));
        assert!(!wire().mentions("power"));
    }

    #[test]
    fn definition_round_trips_through_the_manifest_format() {
        for definition in [log(), fence(), wire()] {
            let json = serde_json::to_string(&definition).unwrap();
            assert_eq!(
                serde_json::from_str::<BlockDefinition>(&json).unwrap(),
                definition
            );
        }
    }

    #[test]
    fn rejects_files_without_variants_or_multipart() {
        assert!(BlockDefinition::from_json(b"{}").is_err());
        assert!(BlockDefinition::from_json(b"not json").is_err());
    }
}
