//! Stub of the `fdoom.network` package — see PORTING.md ("Multiplayer").
//!
//! The Java tree carries a ~1900-line socket-based client/server (`GameServer`,
//! `GameClient`, `GameServerThread`, `GameProtocol`, `GameConnection`) plus the
//! server-console `ConsoleReader`, inherited from Minicraft Plus. This build is
//! singleplayer-only: every Java call site of `Game.ISONLINE` / `isValidServer()` /
//! `isValidClient()` / `isConnectedClient()` is preserved in the port and reaches the
//! stubbed accessors on `Game` (all `false`), so game logic reads exactly like the Java
//! code and a real network layer can be added behind the same functions without touching
//! gameplay. `MultiplayerDisplay` reports that multiplayer is unavailable.
//!
//! `Network.generateUniqueEntityId()` lives on `EntityArena::insert`;
//! `Network.getEntity(eid)` is `g.entities.get(eid)`; `Network.onlinePrefix()` is always
//! the empty string.

/// Java `Network.onlinePrefix()` — always offline here.
pub fn online_prefix() -> &'static str {
    ""
}
