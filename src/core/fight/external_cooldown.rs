//! Go core/buffs.go `registerExternalConsecutiveCDApproximation`, which
//! `NewGeneratedExternalCD` wraps around a generated aura: other players cast a buff on the player
//! on cooldown, `sources` of them taking turns. The cast is a simple spell with no cost, metrics or
//! log on its own timer, which the major cooldown manager uses. It needs the next source's timer
//! ready and no active aura with the buff's tag, then activates the aura, puts that source on
//! cooldown and holds the player's timer until the next source is ready.

use crate::contracts::prepared_v2::{Aura, Effect};

use super::{Agent, AuraRef, Fight, SpellBehavior, SpellId};

/// One external cooldown: Go's closure state.
#[derive(Clone, Debug)]
pub(crate) struct ExternalCooldown {
    spell: SpellId,
    aura: AuraRef,
    /// Every aura of the player with the buff's tag, Go `HasActiveAuraWithTag`.
    tagged: Vec<AuraRef>,
    /// Each source's timer in `Fight::timers`, Go `externalTimers`.
    timers: Vec<usize>,
    /// The player's timer of the cast, Go `sharedTimer`.
    shared: usize,
    cooldown: i64,
    duration: i64,
    /// Go `nextExternalIndex`, a variable of the closure that no reset touches: the source that
    /// casts next carries over from one fight to the next.
    next: usize,
}

impl<A: Agent> Fight<A> {
    /// Bind the external cooldowns: each one's spell, aura, tagged auras and the timers its
    /// sources take, registered after the exported ones as Go's `NewTimer` calls are.
    pub(crate) fn bind_external_cooldowns(
        &mut self,
        effects: &[Effect],
        auras: &[Aura],
    ) -> Result<(), String> {
        for effect in effects {
            let Effect::ExternalCooldown {
                spell_id,
                spell_tag,
                aura,
                aura_tag,
                sources,
                cooldown_ns,
                duration_ns,
            } = effect
            else {
                continue;
            };
            if *sources < 1 {
                return Err("an external cooldown needs at least 1 source".into());
            }
            let spell = self
                .spells
                .iter()
                .position(|spell| {
                    spell.id.spell_id == *spell_id
                        && spell.id.tag == *spell_tag
                        && spell.id.item_id == 0
                        && matches!(spell.behavior, SpellBehavior::ExternalCooldown)
                })
                .ok_or_else(|| format!("the external cooldown {spell_id} has no spell"))?;
            let (shared, spell_duration) = self.spells[spell]
                .cd
                .ok_or_else(|| format!("the external cooldown {spell_id} has no timer"))?;
            if spell_duration != *duration_ns {
                return Err(format!(
                    "the external cooldown {spell_id} waits {spell_duration} ns, not its aura's duration"
                ));
            }
            let aura = self.player_aura(aura)?;
            let mut tagged = Vec::new();
            for exported in auras {
                if exported.tag.as_deref() == Some(aura_tag.as_str()) {
                    tagged.push(self.player_aura(&exported.label)?);
                }
            }
            let mut timers = Vec::new();
            for _ in 0..*sources {
                self.timers.push(crate::core::time::STARTING_CD_TIME);
                timers.push(self.timers.len() - 1);
            }
            self.external_cooldowns.push(ExternalCooldown {
                spell,
                aura,
                tagged,
                timers,
                shared,
                cooldown: *cooldown_ns,
                duration: *duration_ns,
                next: 0,
            });
        }
        Ok(())
    }

    fn external_cooldown_of(&self, spell: SpellId) -> usize {
        self.external_cooldowns
            .iter()
            .position(|external| external.spell == spell)
            .expect("an external cooldown spell is bound")
    }

    /// Go's `ExtraCastCondition`: the next source's timer is ready and no aura with the buff's
    /// tag is active.
    pub(crate) fn external_cooldown_castable(&self, spell: SpellId) -> bool {
        let external = &self.external_cooldowns[self.external_cooldown_of(spell)];
        self.timers[external.timers[external.next]] <= self.now
            && !external.tagged.iter().any(|&aura| self.aura(aura).active)
    }

    /// Go's `ApplyEffects`: the aura activates, the source that cast goes on cooldown and the
    /// player's timer waits for the next source, or for the aura to end if it is ready now.
    pub(crate) fn external_cooldown_cast(&mut self, spell: SpellId) {
        let index = self.external_cooldown_of(spell);
        let external = self.external_cooldowns[index].clone();
        self.activate_aura(external.aura);
        let now = self.now;
        self.timers[external.timers[external.next]] = now + external.cooldown;
        let next = (external.next + 1) % external.timers.len();
        let ready_at = self.timers[external.timers[next]];
        self.timers[external.shared] = if ready_at <= now {
            now + external.duration
        } else {
            now + (ready_at - now).max(0)
        };
        self.external_cooldowns[index].next = next;
    }
}
