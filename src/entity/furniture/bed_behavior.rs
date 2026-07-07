//! Behavior of `fdoom.entity.furniture.Bed`.
//!
//! Java's statics (`playersAwake`, `sleepingPlayers`) live in `g.bed_state`. Java stored
//! `Player -> Bed` references; we store player eid -> (level index, bed eid).

use crate::core::game::Game;
use crate::core::updater::{NORM_SPEED, SLEEP_END_TIME, SLEEP_START_TIME};
use crate::entity::{Entity, behavior};

/// Java `Bed.use(player)` — called when the player attempts to get in bed.
pub fn use_furniture(g: &mut Game, e: &mut Entity, player: &mut Entity) -> bool {
    if check_can_sleep(g, player) {
        // if it is late enough in the day to sleep...

        // set the player spawn coord. to their current position, in tile coords (hence " >> 4")
        let (px, py) = (player.c.x, player.c.y);
        {
            let pd = player.player_mut();
            pd.spawnx = px >> 4;
            pd.spawny = py >> 4;
        }

        // record which bed (level, eid) the player is sleeping in
        let bed_lvl = e.c.level.unwrap_or(g.current_level);
        g.bed_state
            .sleeping_players
            .insert(player.c.eid, (bed_lvl, e.c.eid));
        if g.debug {
            println!("player got in bed: {}", player.c.eid);
        }
        behavior::remove_entity(g, player);

        if !g.is_online() {
            g.bed_state.players_awake = 0;
        }
    }

    true
}

/// Java `Bed.getPlayersAwake()`.
pub fn get_players_awake(g: &Game) -> i32 {
    g.bed_state.players_awake
}

/// Java `Bed.setPlayersAwake(count)` — client-only in Java.
pub fn set_players_awake(g: &mut Game, count: i32) {
    if !g.is_valid_client() {
        panic!("Bed.setPlayersAwake() can only be called on a client runtime");
    }
    g.bed_state.players_awake = count;
}

/// Java `Bed.checkCanSleep(player)`.
pub fn check_can_sleep(g: &mut Game, player: &Entity) -> bool {
    if in_bed(g, player.c.eid) {
        return false;
    }

    if !(g.tick_count >= SLEEP_START_TIME || g.tick_count < SLEEP_END_TIME && g.past_day1) {
        // it is too early to sleep; display how much time is remaining.
        // gets the seconds until sleeping is allowed. // normSpeed is in tiks/sec.
        let sec =
            (((SLEEP_START_TIME - g.tick_count) as f64 * 1.0) / NORM_SPEED as f64).ceil() as i32;
        let note = format!("Can't sleep! {}Min {} Sec left!", sec / 60, sec % 60);
        // add the notification displaying the time remaining in minutes and seconds.
        g.notifications.push(note);

        return false;
    }

    true
}

/// Java `Bed.sleeping()`.
pub fn sleeping(g: &Game) -> bool {
    g.bed_state.players_awake == 0
}

/// Java `Bed.inBed(player)`.
pub fn in_bed(g: &Game, player_eid: i32) -> bool {
    g.bed_state.sleeping_players.contains_key(&player_eid)
}

/// Java `Bed.getBedLevel(player)`.
pub fn get_bed_level(g: &Game, player_eid: i32) -> Option<usize> {
    let (stored_lvl, bed_eid) = *g.bed_state.sleeping_players.get(&player_eid)?;
    // prefer the live bed entity's level; if the bed is gone, fall back to the level
    // recorded when the player went to sleep
    match g.entities.get(bed_eid) {
        Some(bed) => bed.c.level,
        None => Some(stored_lvl),
    }
}

/// Java `Bed.removePlayer(player)` — get the player "out of bed"; used on the client only.
pub fn remove_player(g: &mut Game, player_eid: i32) {
    g.bed_state.sleeping_players.remove(&player_eid);
}

/// Java `Bed.removePlayers()`.
pub fn remove_players(g: &mut Game) {
    g.bed_state.sleeping_players.clear();
}

/// Re-adds a sleeping player to its bed's level (the `bed.getLevel().add(player)` part
/// shared by `restorePlayer`/`restorePlayers`).
fn add_player_to_bed_level(g: &mut Game, player_eid: i32, stored_lvl: usize, bed_eid: i32) {
    // prefer the live bed entity's level; if the bed is gone from the arena, fall back
    // to the level recorded at sleep time
    let lvl = match g.entities.get(bed_eid) {
        Some(bed) => bed.c.level.unwrap_or(g.current_level),
        None => stored_lvl,
    };
    if let Some(player) = g.entities.take(player_eid) {
        g.level_mut(lvl).add(player, lvl);
    }
}

/// Java `Bed.restorePlayer(player)` — client should not call this.
pub fn restore_player(g: &mut Game, player_eid: i32) {
    if let Some((stored_lvl, bed_eid)) = g.bed_state.sleeping_players.remove(&player_eid) {
        add_player_to_bed_level(g, player_eid, stored_lvl, bed_eid);

        if !g.is_online() {
            g.bed_state.players_awake = 1;
        }
    }
}

/// Java `Bed.restorePlayers()` — client should not call this.
pub fn restore_players(g: &mut Game) {
    let sleeping: Vec<(i32, (usize, i32))> = g
        .bed_state
        .sleeping_players
        .iter()
        .map(|(&p, &b)| (p, b))
        .collect();
    for (player_eid, (stored_lvl, bed_eid)) in sleeping {
        add_player_to_bed_level(g, player_eid, stored_lvl, bed_eid);
    }

    g.bed_state.sleeping_players.clear();

    if !g.is_online() {
        g.bed_state.players_awake = 1;
    }
}
