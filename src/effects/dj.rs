use serde::Deserialize;

use std::io;
use std::io::Read;
use std::sync::Arc;
use std::fs::File;
use std::collections::HashMap;
use enum_iterator::IntoEnumIterator;

use rodio;
use rodio::Source;
use std::convert::AsRef;

//---------------------------------------------------

struct SoundImpl (Arc<Vec<u8>>);

impl AsRef<[u8]> for SoundImpl {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl SoundImpl {
    fn load(filename: &str) -> io::Result<SoundImpl> {
        let mut buf = Vec::new();
        let mut file = File::open(filename)?;
        file.read_to_end(&mut buf)?;
        Ok(SoundImpl(Arc::new(buf)))
    }
    fn cursor(self: &Self) -> io::Cursor<SoundImpl> {
        io::Cursor::new(SoundImpl(self.0.clone()))
    }
    fn decoder(self: &Self) -> rodio::Decoder<io::Cursor<SoundImpl>> {
        rodio::Decoder::new(self.cursor()).unwrap()
    }
}

//---------------------------------------------------

lazy_static::lazy_static! {
    static ref KSOUND_PATHS: HashMap<KSnd, String> = {
        let snds_dir = "/opt/ktrl/assets/sounds";
        [
            (KSnd::Click1, format!("{}/click1.wav", snds_dir)),
            (KSnd::Click2, format!("{}/click2.wav", snds_dir)),
            (KSnd::Sticky, format!("{}/sticky.wav", snds_dir)),
            (KSnd::Error, format!("{}/error.wav", snds_dir)),
        ].iter().cloned().collect()
    };
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, IntoEnumIterator, Deserialize)]
pub enum KSnd {
    Click1,
    Click2,
    Sticky,
    Error,
}

pub struct Dj {
    dev: rodio::Device,
    ksnds: HashMap<KSnd, SoundImpl>,
    custom_snds: HashMap<String, SoundImpl>,
}

impl Dj {
    fn make_ksnds() -> HashMap<KSnd, SoundImpl> {
        let mut out: HashMap<KSnd, SoundImpl> = HashMap::new();
        for snd in KSnd::into_enum_iter() {
            let path = &KSOUND_PATHS[&snd];
            out.insert(snd, SoundImpl::load(path).unwrap());
        }
        out
    }

    pub fn new() -> Self {
        let dev = rodio::default_output_device()
            .expect("Failed to open the default sound device");
        let ksnds = Self::make_ksnds();
        Self{dev, ksnds, custom_snds: HashMap::new()}
    }

    pub fn play(&self, snd: KSnd) {
        let snd = &self.ksnds[&snd];
        rodio::play_raw(&self.dev, snd.decoder().convert_samples());
    }

    pub fn play_custom(&mut self, path: &String) {
        if !self.custom_snds.contains_key(path) {
            self.custom_snds.insert(path.clone(), SoundImpl::load(path).unwrap());
        }

        let snd = &self.custom_snds[path];
        rodio::play_raw(&self.dev, snd.decoder().convert_samples());
    }
}
