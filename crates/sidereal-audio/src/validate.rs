use crate::{
    AudioConcurrencyGroup, AudioCueDefinition, AudioEffectDefinition, AudioProfileDefinition,
    AudioRegistry, AudioRegistryError,
};
use std::collections::{BTreeMap, HashSet};

const ROOT_MASTER_BUS_ID: &str = "master";

pub fn validate_audio_registry(registry: &AudioRegistry) -> Result<(), AudioRegistryError> {
    if registry.schema_version < 1 {
        return Err(AudioRegistryError::Contract(
            "schema_version must be >= 1".to_string(),
        ));
    }

    let bus_ids = validate_buses(registry)?;
    let send_ids = validate_effect_collections(
        registry
            .sends
            .iter()
            .map(|send| (send.send_id.as_str(), &send.effects)),
        "sends",
    )?;
    let concurrency_group_ids = validate_concurrency_groups(&registry.concurrency_groups)?;

    validate_environments(registry, &bus_ids, &send_ids)?;
    validate_profiles(registry, &bus_ids, &send_ids, &concurrency_group_ids)?;
    Ok(())
}

fn validate_buses(registry: &AudioRegistry) -> Result<HashSet<String>, AudioRegistryError> {
    let mut bus_ids = HashSet::<String>::new();
    bus_ids.insert(ROOT_MASTER_BUS_ID.to_string());
    for bus in &registry.buses {
        if bus.bus_id.trim().is_empty() {
            return Err(AudioRegistryError::Contract(
                "buses entries must have non-empty bus_id".to_string(),
            ));
        }
        if bus.bus_id == ROOT_MASTER_BUS_ID {
            return Err(AudioRegistryError::Contract(
                "buses must not redefine built-in master bus".to_string(),
            ));
        }
        if !bus_ids.insert(bus.bus_id.clone()) {
            return Err(AudioRegistryError::Contract(format!(
                "buses duplicates bus_id={}",
                bus.bus_id
            )));
        }
    }
    for bus in &registry.buses {
        if let Some(parent) = bus.parent.as_deref() {
            if parent.trim().is_empty() {
                return Err(AudioRegistryError::Contract(format!(
                    "bus {} parent must not be blank",
                    bus.bus_id
                )));
            }
            if parent == bus.bus_id {
                return Err(AudioRegistryError::Contract(format!(
                    "bus {} cannot parent itself",
                    bus.bus_id
                )));
            }
            if !bus_ids.contains(parent) {
                return Err(AudioRegistryError::Contract(format!(
                    "bus {} references unknown parent={}",
                    bus.bus_id, parent
                )));
            }
        }
    }
    Ok(bus_ids)
}

fn validate_effect_collections<'a, I>(
    collections: I,
    context: &str,
) -> Result<HashSet<String>, AudioRegistryError>
where
    I: IntoIterator<Item = (&'a str, &'a Vec<AudioEffectDefinition>)>,
{
    let mut ids = HashSet::<String>::new();
    for (id, effects) in collections {
        if id.trim().is_empty() {
            return Err(AudioRegistryError::Contract(format!(
                "{context} entries must have non-empty ids"
            )));
        }
        if !ids.insert(id.to_string()) {
            return Err(AudioRegistryError::Contract(format!(
                "{context} duplicates id={id}"
            )));
        }
        for (index, effect) in effects.iter().enumerate() {
            if effect.kind.trim().is_empty() {
                return Err(AudioRegistryError::Contract(format!(
                    "{context}.{id}.effects[{}].kind must not be empty",
                    index + 1
                )));
            }
        }
    }
    Ok(ids)
}

fn validate_concurrency_groups(
    groups: &[AudioConcurrencyGroup],
) -> Result<HashSet<String>, AudioRegistryError> {
    let mut group_ids = HashSet::<String>::new();
    for group in groups {
        if group.group_id.trim().is_empty() {
            return Err(AudioRegistryError::Contract(
                "concurrency_groups entries must have non-empty group_id".to_string(),
            ));
        }
        if !group_ids.insert(group.group_id.clone()) {
            return Err(AudioRegistryError::Contract(format!(
                "concurrency_groups duplicates group_id={}",
                group.group_id
            )));
        }
        if group.max_instances == 0 {
            return Err(AudioRegistryError::Contract(format!(
                "concurrency group {} max_instances must be > 0",
                group.group_id
            )));
        }
        if group.scope.trim().is_empty() {
            return Err(AudioRegistryError::Contract(format!(
                "concurrency group {} scope must not be empty",
                group.group_id
            )));
        }
    }
    Ok(group_ids)
}

fn validate_environments(
    registry: &AudioRegistry,
    bus_ids: &HashSet<String>,
    send_ids: &HashSet<String>,
) -> Result<(), AudioRegistryError> {
    let mut environment_ids = HashSet::<String>::new();
    for environment in &registry.environments {
        if environment.environment_id.trim().is_empty() {
            return Err(AudioRegistryError::Contract(
                "environments entries must have non-empty environment_id".to_string(),
            ));
        }
        if !environment_ids.insert(environment.environment_id.clone()) {
            return Err(AudioRegistryError::Contract(format!(
                "environments duplicates environment_id={}",
                environment.environment_id
            )));
        }
        validate_known_keys(
            &environment.bus_overrides,
            bus_ids,
            &format!("environment {}", environment.environment_id),
            "bus override",
        )?;
        validate_known_keys(
            &environment.bus_effect_overrides,
            bus_ids,
            &format!("environment {}", environment.environment_id),
            "bus effect override",
        )?;
        for (bus_id, effects) in &environment.bus_effect_overrides {
            for (index, effect) in effects.iter().enumerate() {
                if effect.kind.trim().is_empty() {
                    return Err(AudioRegistryError::Contract(format!(
                        "environment {} bus {} effect[{}].kind must not be empty",
                        environment.environment_id,
                        bus_id,
                        index + 1
                    )));
                }
            }
        }
        validate_known_keys(
            &environment.send_level_db,
            send_ids,
            &format!("environment {}", environment.environment_id),
            "send level",
        )?;
    }
    Ok(())
}

fn validate_profiles(
    registry: &AudioRegistry,
    bus_ids: &HashSet<String>,
    send_ids: &HashSet<String>,
    concurrency_group_ids: &HashSet<String>,
) -> Result<(), AudioRegistryError> {
    let mut profile_ids = HashSet::<String>::new();
    for profile in &registry.profiles {
        validate_profile_id(profile, &mut profile_ids)?;
        if profile.kind.trim().is_empty() {
            return Err(AudioRegistryError::Contract(format!(
                "profile {} kind must not be empty",
                profile.profile_id
            )));
        }
        if profile.cues.is_empty() {
            return Err(AudioRegistryError::Contract(format!(
                "profile {} must define at least one cue",
                profile.profile_id
            )));
        }
        validate_cues(
            &profile.profile_id,
            &profile.cues,
            bus_ids,
            send_ids,
            concurrency_group_ids,
        )?;
    }
    Ok(())
}

fn validate_profile_id(
    profile: &AudioProfileDefinition,
    profile_ids: &mut HashSet<String>,
) -> Result<(), AudioRegistryError> {
    if profile.profile_id.trim().is_empty() {
        return Err(AudioRegistryError::Contract(
            "profiles entries must have non-empty profile_id".to_string(),
        ));
    }
    if !profile_ids.insert(profile.profile_id.clone()) {
        return Err(AudioRegistryError::Contract(format!(
            "profiles duplicates profile_id={}",
            profile.profile_id
        )));
    }
    Ok(())
}

fn validate_cues(
    profile_id: &str,
    cues: &BTreeMap<String, AudioCueDefinition>,
    bus_ids: &HashSet<String>,
    send_ids: &HashSet<String>,
    concurrency_group_ids: &HashSet<String>,
) -> Result<(), AudioRegistryError> {
    for (cue_id, cue) in cues {
        if cue_id.trim().is_empty() {
            return Err(AudioRegistryError::Contract(format!(
                "profile {} contains blank cue id",
                profile_id
            )));
        }
        validate_cue(
            profile_id,
            cue_id,
            cue,
            bus_ids,
            send_ids,
            concurrency_group_ids,
        )?;
    }
    Ok(())
}

fn validate_cue(
    profile_id: &str,
    cue_id: &str,
    cue: &AudioCueDefinition,
    bus_ids: &HashSet<String>,
    send_ids: &HashSet<String>,
    concurrency_group_ids: &HashSet<String>,
) -> Result<(), AudioRegistryError> {
    if cue.route.bus.trim().is_empty() {
        return Err(AudioRegistryError::Contract(format!(
            "profile {} cue {} route.bus must not be empty",
            profile_id, cue_id
        )));
    }
    if !bus_ids.contains(cue.route.bus.as_str()) {
        return Err(AudioRegistryError::Contract(format!(
            "profile {} cue {} references unknown bus={}",
            profile_id, cue_id, cue.route.bus
        )));
    }
    let mut seen_send_ids = HashSet::<&str>::new();
    for send in &cue.route.sends {
        if send.send_id.trim().is_empty() {
            return Err(AudioRegistryError::Contract(format!(
                "profile {} cue {} route sends must not have blank send_id",
                profile_id, cue_id
            )));
        }
        if !seen_send_ids.insert(send.send_id.as_str()) {
            return Err(AudioRegistryError::Contract(format!(
                "profile {} cue {} duplicates route send={}",
                profile_id, cue_id, send.send_id
            )));
        }
        if !send_ids.contains(send.send_id.as_str()) {
            return Err(AudioRegistryError::Contract(format!(
                "profile {} cue {} references unknown send={}",
                profile_id, cue_id, send.send_id
            )));
        }
    }
    validate_playback(profile_id, cue_id, cue)?;
    validate_spatial(profile_id, cue_id, cue)?;
    if let Some(concurrency) = &cue.concurrency
        && let Some(group_id) = concurrency.group_id.as_deref()
        && !concurrency_group_ids.contains(group_id)
    {
        return Err(AudioRegistryError::Contract(format!(
            "profile {} cue {} references unknown concurrency group={}",
            profile_id, cue_id, group_id
        )));
    }
    Ok(())
}

fn validate_playback(
    profile_id: &str,
    cue_id: &str,
    cue: &AudioCueDefinition,
) -> Result<(), AudioRegistryError> {
    let playback = &cue.playback;
    if playback.kind.trim().is_empty() {
        return Err(AudioRegistryError::Contract(format!(
            "profile {} cue {} playback.kind must not be empty",
            profile_id, cue_id
        )));
    }
    let has_clip = playback
        .clip_asset_id
        .as_deref()
        .is_some_and(|clip| !clip.trim().is_empty());
    let has_variants = !playback.variants.is_empty();
    if !has_clip && !has_variants {
        return Err(AudioRegistryError::Contract(format!(
            "profile {} cue {} must define clip_asset_id or variants",
            profile_id, cue_id
        )));
    }
    for (index, variant) in playback.variants.iter().enumerate() {
        if variant.clip_asset_id.trim().is_empty() {
            return Err(AudioRegistryError::Contract(format!(
                "profile {} cue {} variants[{}].clip_asset_id must not be empty",
                profile_id,
                cue_id,
                index + 1
            )));
        }
        if !(variant.weight.is_finite() && variant.weight > 0.0) {
            return Err(AudioRegistryError::Contract(format!(
                "profile {} cue {} variants[{}].weight must be > 0",
                profile_id,
                cue_id,
                index + 1
            )));
        }
    }
    if let Some(loop_region) = &playback.loop_region
        && !(loop_region.start_s.is_finite()
            && loop_region.end_s.is_finite()
            && loop_region.start_s >= 0.0
            && loop_region.end_s > loop_region.start_s)
    {
        return Err(AudioRegistryError::Contract(format!(
            "profile {} cue {} loop_region must satisfy 0 <= start < end",
            profile_id, cue_id
        )));
    }
    if playback.kind == "segmented_loop" {
        let required = [
            ("intro_start_s", playback.intro_start_s),
            ("loop_start_s", playback.loop_start_s),
            ("loop_end_s", playback.loop_end_s),
            ("outro_start_s", playback.outro_start_s),
            ("clip_end_s", playback.clip_end_s),
        ];
        for (name, value) in required {
            if value.is_none() {
                return Err(AudioRegistryError::Contract(format!(
                    "profile {} cue {} segmented_loop missing {}",
                    profile_id, cue_id, name
                )));
            }
        }
        let intro = playback.intro_start_s.unwrap_or_default();
        let loop_start = playback.loop_start_s.unwrap_or_default();
        let loop_end = playback.loop_end_s.unwrap_or_default();
        let outro = playback.outro_start_s.unwrap_or_default();
        let clip_end = playback.clip_end_s.unwrap_or_default();
        if !(intro >= 0.0
            && intro <= loop_start
            && loop_start < loop_end
            && loop_end <= outro
            && outro < clip_end)
        {
            return Err(AudioRegistryError::Contract(format!(
                "profile {} cue {} segmented_loop markers must satisfy intro <= loop_start < loop_end <= outro < clip_end",
                profile_id, cue_id
            )));
        }
    }
    Ok(())
}

fn validate_spatial(
    profile_id: &str,
    cue_id: &str,
    cue: &AudioCueDefinition,
) -> Result<(), AudioRegistryError> {
    let spatial = &cue.spatial;
    if spatial.mode.trim().is_empty() {
        return Err(AudioRegistryError::Contract(format!(
            "profile {} cue {} spatial.mode must not be empty",
            profile_id, cue_id
        )));
    }
    if let (Some(min_distance_m), Some(max_distance_m)) =
        (spatial.min_distance_m, spatial.max_distance_m)
        && (!(min_distance_m.is_finite() && max_distance_m.is_finite())
            || min_distance_m < 0.0
            || max_distance_m <= min_distance_m)
    {
        return Err(AudioRegistryError::Contract(format!(
            "profile {} cue {} spatial distances must satisfy 0 <= min < max",
            profile_id, cue_id
        )));
    }
    if let Some(lowpass) = &spatial.distance_lowpass
        && let (Some(near_hz), Some(far_hz)) = (lowpass.near_hz, lowpass.far_hz)
        && (!(near_hz.is_finite() && far_hz.is_finite()) || near_hz <= far_hz || far_hz <= 0.0)
    {
        return Err(AudioRegistryError::Contract(format!(
            "profile {} cue {} distance_lowpass must satisfy near_hz > far_hz > 0",
            profile_id, cue_id
        )));
    }
    Ok(())
}

fn validate_known_keys<T>(
    values: &BTreeMap<String, T>,
    known_ids: &HashSet<String>,
    context: &str,
    label: &str,
) -> Result<(), AudioRegistryError> {
    for key in values.keys() {
        if key.trim().is_empty() {
            return Err(AudioRegistryError::Contract(format!(
                "{context} has blank {label} key"
            )));
        }
        if !known_ids.contains(key) {
            return Err(AudioRegistryError::Contract(format!(
                "{context} references unknown {label}={key}"
            )));
        }
    }
    Ok(())
}
