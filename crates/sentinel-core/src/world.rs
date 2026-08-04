use super::bomb::BombState;
use super::evidence::Evidence;
use super::feature::FeatureVector;
use super::grenade::GrenadeState;
use super::kill::KillEvent;
use super::player::{PlayerId, PlayerState, Team};
use super::round::RoundState;
use super::tick::{Tick, TickState};
use sentinel_map::MapData;

/// Match context provides read-only access to all match data.
/// This is passed to feature extractors and analyzers.
pub struct MatchContext {
    /// All tick states in order (sorted by tick)
    states: Vec<TickState>,
    /// Feature vectors computed for all players
    feature_vectors: Vec<FeatureVector>,
    /// Evidence collected during analysis
    evidence: Vec<Evidence>,
    /// Map data for visibility calculations
    map: MapData,
    /// All kill events (sorted by tick), stored once instead of per-TickState
    kills: Vec<KillEvent>,
}

impl MatchContext {
    pub fn new(states: Vec<TickState>) -> Self {
        Self {
            states,
            feature_vectors: Vec::new(),
            evidence: Vec::new(),
            map: MapData::dust2(),
            kills: Vec::new(),
        }
    }

    /// Set the map data for visibility calculations
    pub fn set_map(&mut self, map: MapData) {
        self.map = map;
    }

    /// Get the map data
    pub fn map(&self) -> &MapData {
        &self.map
    }

    /// Set all kill events (must be sorted by tick)
    pub fn set_kills(&mut self, kills: Vec<KillEvent>) {
        self.kills = kills;
    }

    /// Get all kill events
    pub fn kills(&self) -> &[KillEvent] {
        &self.kills
    }

    /// Get all kills up to and including a specific tick (O(log n) via binary search)
    pub fn kills_up_to(&self, tick: Tick) -> &[KillEvent] {
        let idx = self
            .kills
            .partition_point(|k| k.tick.0 <= tick.0);
        &self.kills[..idx]
    }

    /// Get the state at a specific tick using binary search (O(log n))
    pub fn state_at(&self, tick: Tick) -> Option<&TickState> {
        self.states
            .binary_search_by_key(&tick, |s| s.tick)
            .ok()
            .map(|idx| &self.states[idx])
    }

    /// Get all tick states in a range [from, to] (inclusive) using binary search
    pub fn states_in_range(&self, from: Tick, to: Tick) -> &[TickState] {
        if self.states.is_empty() {
            return &[];
        }

        let start_idx = self
            .states
            .binary_search_by_key(&from, |s| s.tick)
            .unwrap_or_else(|i| i);

        let end_idx = match self.states.binary_search_by_key(&to, |s| s.tick) {
            Ok(i) => i + 1,
            Err(i) => i,
        };

        if start_idx >= end_idx {
            return &[];
        }

        &self.states[start_idx..end_idx.min(self.states.len())]
    }

    /// Get all tick states
    pub fn states(&self) -> &[TickState] {
        &self.states
    }

    /// Get the first tick
    pub fn first_tick(&self) -> Tick {
        self.states.first().map(|s| s.tick).unwrap_or(Tick(0))
    }

    /// Get the last tick
    pub fn last_tick(&self) -> Tick {
        self.states.last().map(|s| s.tick).unwrap_or(Tick(0))
    }

    /// Get total number of ticks
    pub fn tick_count(&self) -> usize {
        self.states.len()
    }

    /// Get current round number (from the last state)
    pub fn current_round(&self) -> u32 {
        self.states
            .last()
            .map(|s| s.round.round_number)
            .unwrap_or(0)
    }

    /// Get all alive players at a tick
    pub fn alive_players_at(&self, tick: Tick) -> Vec<&PlayerState> {
        self.state_at(tick)
            .map(|s| s.players.iter().filter(|p| p.alive).collect())
            .unwrap_or_default()
    }

    /// Get a specific player at a tick
    pub fn player_at(&self, tick: Tick, player_id: PlayerId) -> Option<&PlayerState> {
        self.state_at(tick)?
            .players
            .iter()
            .find(|p| p.id == player_id)
    }

    /// Get all players on a team at a tick
    pub fn team_players_at(&self, tick: Tick, team: Team) -> Vec<&PlayerState> {
        self.state_at(tick)
            .map(|s| {
                s.players
                    .iter()
                    .filter(|p| p.team == team && p.alive)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get the bomb state at a tick
    pub fn bomb_at(&self, tick: Tick) -> Option<&BombState> {
        self.state_at(tick).map(|s| &s.bomb)
    }

    /// Get all active grenades at a tick
    pub fn grenades_at(&self, tick: Tick) -> Vec<&GrenadeState> {
        self.state_at(tick)
            .map(|s| s.grenades.iter().filter(|g| g.active).collect())
            .unwrap_or_default()
    }

    /// Get round state at a tick
    pub fn round_at(&self, tick: Tick) -> Option<&RoundState> {
        self.state_at(tick).map(|s| &s.round)
    }

    /// Add a feature vector
    pub fn add_feature_vector(&mut self, fv: FeatureVector) {
        self.feature_vectors.push(fv);
    }

    /// Get all feature vectors for a player
    pub fn feature_vectors_for(&self, player: PlayerId) -> Vec<&FeatureVector> {
        self.feature_vectors
            .iter()
            .filter(|fv| fv.player == player)
            .collect()
    }

    /// Add evidence
    pub fn add_evidence(&mut self, ev: Evidence) {
        self.evidence.push(ev);
    }

    /// Get all evidence
    pub fn evidence(&self) -> &[Evidence] {
        &self.evidence
    }

    /// Get evidence for a specific player
    pub fn evidence_for(&self, player: PlayerId) -> Vec<&Evidence> {
        self.evidence
            .iter()
            .filter(|e| e.player == player)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RoundPhase, RoundState};

    fn make_state(tick: u32) -> TickState {
        TickState {
            tick: Tick(tick),
            players: vec![],
            grenades: vec![],
            bomb: BombState::Carried { carrier: PlayerId::new(0) },
            round: RoundState {
                round_number: 1,
                phase: RoundPhase::Live,
                clock: 100.0,
                t_score: 0,
                ct_score: 0,
                winner: None,
                start_tick: 0,
            },
        }
    }

    fn make_context() -> MatchContext {
        let states = vec![
            make_state(0),
            make_state(100),
            make_state(200),
            make_state(300),
            make_state(400),
        ];
        MatchContext::new(states)
    }

    #[test]
    fn states_in_range_exact_match_boundary() {
        let ctx = make_context();
        // Exact match on both ends — both should be included
        let slice = ctx.states_in_range(Tick(100), Tick(300));
        assert_eq!(slice.len(), 3, "Should include ticks 100, 200, 300");
        assert_eq!(slice[0].tick, Tick(100));
        assert_eq!(slice[1].tick, Tick(200));
        assert_eq!(slice[2].tick, Tick(300));
    }

    #[test]
    fn states_in_range_no_match() {
        let ctx = make_context();
        // No exact match for 150 — should start at next higher (200)
        let slice = ctx.states_in_range(Tick(150), Tick(250));
        assert_eq!(slice.len(), 1);
        assert_eq!(slice[0].tick, Tick(200));
    }

    #[test]
    fn states_in_range_empty_range() {
        let ctx = make_context();
        // from > to in the data
        let slice = ctx.states_in_range(Tick(500), Tick(600));
        assert!(slice.is_empty());
    }

    #[test]
    fn states_in_range_empty_states() {
        let ctx = MatchContext::new(vec![]);
        let slice = ctx.states_in_range(Tick(0), Tick(100));
        assert!(slice.is_empty());
    }
}
