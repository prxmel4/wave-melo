//! Helpers for tagged repeat copies that Lavalink's own queue auto-advance plays.

use std::collections::VecDeque;

use lavalink_rs::model::track::TrackData;
use lavalink_rs::player_context::TrackInQueue;
use serde_json::{json, Value};

/// User-data key marking an internally injected track-repeat copy.
pub const LOOP_MARKER: &str = "wavemelo_loop";
/// User-data key marking the last track of an injected queue-repeat cycle.
pub const QUEUE_CYCLE_MARKER: &str = "wavemelo_queue_cycle";

fn marker(user_data: Option<&Value>, key: &str) -> bool {
    user_data
        .and_then(|data| data.get(key))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Return true when a track is an internal track-repeat copy.
pub fn has_loop_marker(user_data: Option<&Value>) -> bool {
    marker(user_data, LOOP_MARKER)
}

/// Return true when a track ends an internal queue-repeat cycle.
pub fn has_queue_cycle_marker(user_data: Option<&Value>) -> bool {
    marker(user_data, QUEUE_CYCLE_MARKER)
}

fn with_marker(track: &TrackData, key: &str) -> TrackData {
    let mut copy = track.clone();
    let mut data = copy
        .user_data
        .clone()
        .filter(|value| value.is_object())
        .unwrap_or_else(|| json!({}));
    data[key] = Value::Bool(true);
    copy.user_data = Some(data);
    copy
}

/// Clone a track and mark it as an internal repeat copy.
pub fn tag_loop_copy(track: &TrackData) -> TrackData {
    with_marker(track, LOOP_MARKER)
}

/// Build a queue entry for an internal track-repeat copy.
pub fn loop_copy(track: &TrackData) -> TrackInQueue {
    tag_loop_copy(track).into()
}

/// Build one queue-repeat cycle from the current track and queued entries.
///
/// The final entry is tagged so an event handler can replenish the cycle when
/// it finishes. Existing loop markers are never copied.
pub fn build_queue_cycle(
    current: Option<&TrackData>,
    queued: &VecDeque<TrackInQueue>,
) -> VecDeque<TrackInQueue> {
    let mut cycle = VecDeque::new();
    if let Some(track) = current {
        cycle.push_back(track.clone().into());
    }
    for item in queued {
        let data = item.track.user_data.as_ref();
        if !has_loop_marker(data) && !has_queue_cycle_marker(data) {
            cycle.push_back(item.clone());
        }
    }
    if let Some(last) = cycle.back_mut() {
        last.track = with_marker(&last.track, QUEUE_CYCLE_MARKER);
    }
    cycle
}

/// Remove both kinds of internal loop markers from a queue snapshot.
pub fn strip_loop_markers(queue: &mut VecDeque<TrackInQueue>) -> usize {
    let before = queue.len();
    queue.retain(|item| {
        let data = item.track.user_data.as_ref();
        !has_loop_marker(data) && !has_queue_cycle_marker(data)
    });
    before - queue.len()
}

#[cfg(test)]
mod tests {
    use super::{
        build_queue_cycle, has_loop_marker, has_queue_cycle_marker, loop_copy, strip_loop_markers,
        tag_loop_copy, LOOP_MARKER, QUEUE_CYCLE_MARKER,
    };
    use lavalink_rs::model::track::TrackData;
    use lavalink_rs::player_context::TrackInQueue;
    use serde_json::json;
    use std::collections::VecDeque;

    fn sample_track(title: &str) -> TrackData {
        let mut track = TrackData {
            encoded: title.to_owned(),
            ..TrackData::default()
        };
        track.info.title = title.to_owned();
        track.info.author = "artist".to_owned();
        track.info.length = 1000;
        track
    }

    #[test]
    fn detects_only_explicit_true_markers() {
        assert!(has_loop_marker(Some(&json!({ LOOP_MARKER: true }))));
        assert!(!has_loop_marker(Some(&json!({ LOOP_MARKER: false }))));
        assert!(!has_loop_marker(Some(&json!({ "other": 1 }))));
        assert!(!has_loop_marker(None));
        assert!(has_queue_cycle_marker(Some(
            &json!({ QUEUE_CYCLE_MARKER: true })
        )));
        assert!(!has_queue_cycle_marker(Some(
            &json!({ QUEUE_CYCLE_MARKER: false })
        )));
    }

    #[test]
    fn tagging_preserves_requester_metadata() {
        let mut track = sample_track("title");
        track.user_data = Some(json!({"requester": "listener"}));

        let copy = tag_loop_copy(&track);
        let data = copy.user_data.as_ref().unwrap();
        assert_eq!(data["requester"], json!("listener"));
        assert_eq!(data[LOOP_MARKER], json!(true));
    }

    #[test]
    fn queue_cycle_marks_only_its_final_entry() {
        let current = sample_track("current");
        let queued = VecDeque::from(vec![
            TrackInQueue::from(sample_track("next")),
            loop_copy(&sample_track("old-copy")),
        ]);

        let cycle = build_queue_cycle(Some(&current), &queued);
        assert_eq!(cycle.len(), 2);
        assert!(!has_loop_marker(cycle[0].track.user_data.as_ref()));
        assert!(has_queue_cycle_marker(cycle[1].track.user_data.as_ref()));
    }

    #[test]
    fn stripping_removes_both_marker_kinds() {
        let plain: TrackInQueue = sample_track("plain").into();
        let copy = loop_copy(&sample_track("copy"));
        let mut cycle =
            build_queue_cycle(None, &VecDeque::from(vec![sample_track("cycle").into()]));
        let mut queue = VecDeque::from(vec![plain, copy]);
        queue.append(&mut cycle);

        assert_eq!(strip_loop_markers(&mut queue), 2);
        assert_eq!(queue.len(), 1);
    }
}
