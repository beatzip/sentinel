use super::bomb::BombState;
use super::evidence::Evidence;
use super::feature::FeatureVector;
use super::grenade::GrenadeState;
use super::player::{PlayerId, PlayerState, Team};
use super::round::RoundState;
use super::tick::{Tick, TickState};

/// Match context provides read-only access to all match data.
/// This is passed to feature extractors and analyzers.
pub struct MatchContext {
    /// All tick states in order
    states: Vec<TickState>,
    /// Feature vectors computed for all players
    feature_vectors: Vec<FeatureVector>,
    /// Evidence collected during analysis
    evidence: Vec<Evidence>,
}

impl MatchContext {
    pub fn new(states: Vec<TickState>) -> Self {
        Self {
            states,
            feature_vectors: Vec::new(),
            evidence: Vec::new(),
        }
    }

    /// Get the state at a specific tick
    pub fn state_at(&self, tick: Tick) -> Option<&TickState> {
        self.states.iter().find(|s| s.tick == tick)
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
