//! Port of `fdoom.core.io.Sound` — the game's sound effects, played through rodio
//! (Java used `javax.sound.sampled.Clip`).
//!
//! Java semantics preserved: each sound has a single playback channel; `play` restarts the
//! clip if it is already playing; `loop` starts/stops continuous looping.

use std::io::Cursor;

use rodio::Source;

use crate::assets;

/// One enum value per Java `Sound` static field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    PlayerHurt,
    PlayerDeath,
    MonsterHurt,
    BossDeath,
    Fuse,
    Explode,
    Pickup,
    Craft,
    Back,
    Select,
    Confirm,
}

impl Sound {
    const ALL: [Sound; 11] = [
        Sound::PlayerHurt,
        Sound::PlayerDeath,
        Sound::MonsterHurt,
        Sound::BossDeath,
        Sound::Fuse,
        Sound::Explode,
        Sound::Pickup,
        Sound::Craft,
        Sound::Back,
        Sound::Select,
        Sound::Confirm,
    ];

    fn wav_bytes(self) -> &'static [u8] {
        match self {
            Sound::PlayerHurt => assets::SOUND_PLAYER_HURT,
            Sound::PlayerDeath => assets::SOUND_PLAYER_DEATH,
            Sound::MonsterHurt => assets::SOUND_MONSTER_HURT,
            Sound::BossDeath => assets::SOUND_BOSS_DEATH,
            Sound::Fuse => assets::SOUND_FUSE,
            Sound::Explode => assets::SOUND_EXPLODE,
            Sound::Pickup => assets::SOUND_PICKUP,
            Sound::Craft => assets::SOUND_CRAFT,
            Sound::Back => assets::SOUND_CRAFT, // JAVA: back reuses craft.wav
            Sound::Select => assets::SOUND_SELECT,
            Sound::Confirm => assets::SOUND_CONFIRM,
        }
    }

    fn index(self) -> usize {
        Sound::ALL.iter().position(|&s| s == self).unwrap()
    }
}

type Samples = rodio::source::Buffered<rodio::Decoder<Cursor<&'static [u8]>>>;

struct Channel {
    samples: Samples,
    sink: rodio::Player,
}

pub struct SoundPlayer {
    /// None when audio is unavailable (headless/tests or no output device).
    output: Option<(rodio::MixerDeviceSink, Vec<Channel>)>,
}

impl SoundPlayer {
    /// Open the default output device and pre-decode all clips (Java `Sound.init()` +
    /// the static constructors). Falls back to silent mode on failure.
    pub fn new(has_gui: bool) -> SoundPlayer {
        if !has_gui {
            return SoundPlayer { output: None };
        }
        let stream = match rodio::DeviceSinkBuilder::open_default_sink() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("could not open audio output: {e}");
                return SoundPlayer { output: None };
            }
        };
        let mut channels = Vec::with_capacity(Sound::ALL.len());
        for sound in Sound::ALL {
            let decoder = match rodio::Decoder::new(Cursor::new(sound.wav_bytes())) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("could not load sound file {sound:?}: {e}");
                    return SoundPlayer { output: None };
                }
            };
            let samples = decoder.buffered();
            let sink = rodio::Player::connect_new(stream.mixer());
            channels.push(Channel { samples, sink });
        }
        SoundPlayer { output: Some((stream, channels)) }
    }

    /// Silent player (used by tests and `--server` mode).
    pub fn silent() -> SoundPlayer {
        SoundPlayer { output: None }
    }

    /// Java `Sound.xyz.play()`. `enabled` is the "sound" setting.
    pub fn play(&self, sound: Sound, enabled: bool) {
        if !enabled {
            return;
        }
        if let Some((_, channels)) = &self.output {
            let ch = &channels[sound.index()];
            // JAVA: if the clip is running, stop and restart from the beginning.
            ch.sink.stop();
            ch.sink.append(ch.samples.clone());
            ch.sink.play();
        }
    }

    /// Java `Sound.xyz.loop(start)`.
    pub fn play_loop(&self, sound: Sound, start: bool, enabled: bool) {
        if !enabled {
            return;
        }
        if let Some((_, channels)) = &self.output {
            let ch = &channels[sound.index()];
            ch.sink.stop();
            if start {
                ch.sink.append(ch.samples.clone().repeat_infinite());
                ch.sink.play();
            }
        }
    }
}
